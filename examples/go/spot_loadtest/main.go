// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://openpit.dev and the OWNERS file for details.

// spot_loadtest measures the Go FFI submit->decision latency for the
// openpit pre-trade engine running a spot-limit funds policy at high offered
// rates. Run with:
//
//	./spot_loadtest -config configs/baseline.ini
//
// See README.md for the full build and run recipe.
package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"time"

	"openpit-loadtest-spot-funds-go/internal/config"
	"openpit-loadtest-spot-funds-go/internal/driver"
	"openpit-loadtest-spot-funds-go/internal/env"
	"openpit-loadtest-spot-funds-go/internal/generator"
	"openpit-loadtest-spot-funds-go/internal/progress"
	"openpit-loadtest-spot-funds-go/internal/reporter"
)

func main() {
	log.SetFlags(0)

	configPath := flag.String("config", "", "path to the INI config file (required)")
	allowDebugCore := flag.Bool("allow-debug-core", false,
		"allow a debug-built core; latency numbers will be meaningless — for development only")
	showProgress := flag.Bool("progress", true,
		"show live progress on stderr while the run proceeds (default on)")
	flag.Parse()

	if *configPath == "" {
		log.Fatal("error: -config is required\n" +
			"usage: spot_loadtest -config <path/to/config.ini> [-allow-debug-core] [-progress=false]\n" +
			"  the default config is configs/baseline.ini")
	}

	// Resolve the config path; also used to find the repo root.
	absConfig, err := filepath.Abs(*configPath)
	if err != nil {
		log.Fatalf("error: resolve config path %q: %v", *configPath, err)
	}

	cfg, err := config.Load(absConfig)
	if err != nil {
		log.Fatalf("error: %v", err)
	}

	// Repo root is four levels up from examples/go/spot_loadtest/.
	repoRoot := repoRootFromExe()
	e := env.Capture(repoRoot)

	if err := runDebugCoreGuard(e.Core, *allowDebugCore); err != nil {
		log.Fatal(err)
	}

	// Build the pre-materialised deterministic event stream from the config.
	// This is purely CPU work (no FFI); failures are config/logic errors.
	log.SetOutput(os.Stderr) // ensure generator progress goes to stderr
	fmt.Fprintln(os.Stderr, "Generating event stream...")
	stream, err := generator.Generate(cfg)
	if err != nil {
		log.Fatalf("error: generator: %v", err)
	}
	fmt.Fprintf(os.Stderr, "Stream ready: %d order-checks, %d settlements, %d fundings\n",
		stream.Stats.OrderChecks, stream.Stats.Settlements, stream.Stats.Fundings)

	// Build the driver config from the app config.
	driverCfg := driver.FromAppConfig(cfg)

	// Wire the live-counter source for the progress reporter. The LiveSource is
	// populated by driver.Run before any goroutine starts (atomic store); the
	// progress goroutine can call Counters() concurrently at any time after that.
	liveSrc := driver.NewLiveSource()
	driverCfg.Live = liveSrc

	// Start the progress reporter on stderr if enabled. It runs in the background
	// and stops when we call Stop(), which also erases its line so the stdout
	// report is not contaminated by any trailing progress noise.
	//
	// Stdout/stderr separation guarantee: progress writes ONLY to os.Stderr;
	// the reporter writes ONLY to os.Stdout. They are never swapped.
	var prog *progress.Reporter
	if *showProgress {
		// Estimate total decided ops = order-checks + settlements.
		totalDecided := stream.Stats.OrderChecks + stream.Stats.Settlements
		prog = progress.New(os.Stderr, liveSrc, totalDecided, progress.DefaultInterval)
		prog.Start(time.Now())
	}

	// Run the driver: open-loop submission + collection + oracle checks.
	// This blocks until every operation has resolved and the engine has stopped.
	_, snap, err := driver.Run(context.Background(), stream, driverCfg)

	// Stop progress before any output to stdout to guarantee no interleaving.
	if prog != nil {
		prog.Stop()
	}

	// An INVALID run must NEVER print a headline number. A run is invalid when it
	// hit dispatch backpressure (ErrQueueLimit) or produced a zero anti-DCE
	// checksum on a non-empty run (decisions not provably consumed). A HARNESS
	// handoff stall is NOT an invalidity trigger — the handoff is non-blocking and
	// off the measured path, so it never contaminates the headline; it is reported
	// as a diagnostic only. For an invalid run, print the invalid-run report
	// (banner + non-latency diagnostics, NO headline/percentiles) to stdout and
	// exit non-zero. Every OTHER error keeps the original behaviour (fatal log, no
	// report). Run returns the Snapshot alongside the sentinel so the diagnostics
	// (environment, workload counts, dispatch sizing, backpressure, handoff stalls)
	// can still be shown.
	if errors.Is(err, driver.ErrBackpressureInvalidRun) ||
		errors.Is(err, driver.ErrZeroChecksumInvalidRun) {
		reporter.WriteInvalid(os.Stdout, e, cfg, *configPath, snap, stream.Stats)
		fmt.Fprintf(os.Stderr,
			"\nerror: run invalid — %v; latency numbers suppressed\n", err)
		os.Exit(1)
	}
	if err != nil {
		log.Fatalf("error: driver: %v", err)
	}

	// Write the full report to stdout. Nothing else writes to stdout.
	reporter.Write(os.Stdout, e, cfg, *configPath, snap, stream.Stats)
}

// runDebugCoreGuard refuses to continue when the linked core was built with
// debug settings, because FFI latency numbers from such a build are
// meaningless. Pass -allow-debug-core to override (for development use only).
func runDebugCoreGuard(profile env.CoreBuildProfile, allow bool) error {
	if !profile.IsDebug() {
		return nil
	}
	msg := fmt.Sprintf(
		"error: the loaded native core appears to be a debug build "+
			"(profile=%s, opt_level=%s, debug_assertions=%v).\n"+
			"FFI latency numbers from a debug core are meaningless; "+
			"build the core in release mode first:\n\n"+
			"  cargo build --release\n\n"+
			"Pass -allow-debug-core to override (development only).",
		profile.Profile, profile.OptLevel, profile.DebugAssertions,
	)
	if !allow {
		return fmt.Errorf("%s", msg)
	}
	// Override: print a prominent warning to stderr and continue.
	fmt.Fprintf(os.Stderr,
		"\n*** WARNING: running with a debug core (-allow-debug-core). ***\n"+
			"*** Latency numbers are NOT meaningful. ***\n\n"+
			"%s\n\n", msg)
	return nil
}

// repoRootFromExe walks up from the running executable to find the repo root
// (the directory containing the pit Cargo workspace). Falls back to the
// current working directory when it cannot be determined.
func repoRootFromExe() string {
	exe, err := os.Executable()
	if err != nil {
		return cwd()
	}
	exe, err = filepath.EvalSymlinks(exe)
	if err != nil {
		return cwd()
	}

	// The example binary lives at <repo>/examples/go/spot_loadtest/
	// so the repo root is four levels up.
	root := exe
	for i := 0; i < 4; i++ {
		root = filepath.Dir(root)
	}

	if isSystemTemp(root) {
		return cwd()
	}
	return root
}

func cwd() string {
	d, err := os.Getwd()
	if err != nil {
		return "."
	}
	return d
}

func isSystemTemp(p string) bool {
	tmp := os.TempDir()
	if tmp == "" {
		return false
	}
	rel, err := filepath.Rel(tmp, p)
	if err != nil {
		return false
	}
	return len(rel) >= 2 && rel[:2] != ".."
}
