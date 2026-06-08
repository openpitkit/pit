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

// Package env captures the host, runtime, pit repository, and core build
// profile for the load-test environment block. All fields that cannot be read
// on the current platform become "unknown" rather than hard-failing; the debug-
// core guard is the only operation that can refuse to proceed.
package env

import (
	"fmt"
	"os/exec"
	"runtime"
	"strings"

	"github.com/shirou/gopsutil/v4/cpu"
	"github.com/shirou/gopsutil/v4/host"
	"github.com/shirou/gopsutil/v4/mem"
	"go.openpit.dev/openpit"
)

const (
	unknown = "unknown"
	// bytesPerGiB is used to convert OS-reported byte counts to gibibytes.
	bytesPerGiB = 1024 * 1024 * 1024
)

// Host summarizes the physical machine.
type Host struct {
	CPUModel string
	Cores    int
	RAM      string // e.g. "32.0 GiB"
	OS       string // e.g. "macOS 14.5"
	Kernel   string // e.g. "Darwin 24.5.0"
}

// GoRuntime summarizes the Go toolchain.
type GoRuntime struct {
	Version    string
	GOOS       string
	GOARCH     string
	CGOEnabled bool
}

// PitRepo summarizes the pit monorepo revision.
//
// The working-tree status is TRI-STATE: clean | dirty | unknown. Dirty alone
// cannot express "unknown" — a build whose status could not be checked (git
// unavailable, not a repository, command error) must never be reported as
// "clean". DirtyKnown gates Dirty: when DirtyKnown is false the status is
// "unknown" regardless of Dirty; only when DirtyKnown is true does Dirty
// distinguish clean (false) from dirty (true). Use DirtyStatus() to render it.
type PitRepo struct {
	Commit string
	// Dirty is meaningful ONLY when DirtyKnown is true: true => dirty, false =>
	// clean. When DirtyKnown is false the working-tree status is "unknown".
	Dirty bool
	// DirtyKnown is true only when git successfully reported the working-tree
	// status. It defaults to false so an unauditable build is "unknown", never
	// falsely "clean".
	DirtyKnown bool
}

// DirtyStatus renders the tri-state working-tree status for the environment
// report: "clean", "dirty", or "unknown" (git unavailable / not a repo /
// command error). It never returns "clean" for an unchecked tree.
func (p PitRepo) DirtyStatus() string {
	if !p.DirtyKnown {
		return unknown
	}
	if p.Dirty {
		return "dirty"
	}
	return "clean"
}

// CoreBuildProfile is the parsed build profile of the linked native runtime.
// All fields come from the key=value; string returned by GetBuildProfile().
type CoreBuildProfile struct {
	Version         string
	Profile         string // "release" or "debug"
	OptLevel        string // "0", "1", "2", "3", "s", "z"
	DebugAssertions bool   // true if "true"
	Target          string
	TargetCPU       string
	LTO             string
	// Raw is the unparsed key=value; string for the report.
	Raw string
}

// IsDebug returns true when the core was built without optimizations or with
// debug assertions enabled — conditions that make latency numbers meaningless.
func (p CoreBuildProfile) IsDebug() bool {
	return p.DebugAssertions || p.OptLevel == "0" || p.Profile == "debug"
}

// Env is the complete environment snapshot captured before the run starts.
type Env struct {
	Host    Host
	Runtime GoRuntime
	Pit     PitRepo
	Core    CoreBuildProfile
}

// Capture collects all environment fields. It never returns a hard error for
// individual fields; only the core profile parse is fatal-free — errors are
// surfaced as "unknown" field values.
func Capture(repoRoot string) Env {
	e := Env{}
	e.Host = captureHost()
	e.Runtime = captureGoRuntime()
	e.Pit = capturePitRepo(repoRoot)
	e.Core = captureCore()
	return e
}

func captureHost() Host {
	h := Host{
		Cores:    runtime.NumCPU(),
		RAM:      unknown,
		OS:       unknown,
		Kernel:   unknown,
		CPUModel: unknown,
	}

	// CPU model
	infos, err := cpu.Info()
	if err == nil && len(infos) > 0 {
		model := strings.TrimSpace(infos[0].ModelName)
		if model != "" {
			h.CPUModel = model
		}
	}

	// RAM
	vmStat, err := mem.VirtualMemory()
	if err == nil && vmStat.Total > 0 {
		h.RAM = fmt.Sprintf("%.1f GiB", float64(vmStat.Total)/bytesPerGiB)
	}

	// OS + kernel
	hostInfo, err := host.Info()
	if err == nil {
		if hostInfo.Platform != "" || hostInfo.PlatformVersion != "" {
			h.OS = strings.TrimSpace(hostInfo.Platform + " " + hostInfo.PlatformVersion)
		}
		if hostInfo.KernelVersion != "" {
			h.Kernel = strings.TrimSpace(hostInfo.OS + " " + hostInfo.KernelVersion)
		}
	}

	return h
}

func captureGoRuntime() GoRuntime {
	return GoRuntime{
		Version:    runtime.Version(),
		GOOS:       runtime.GOOS,
		GOARCH:     runtime.GOARCH,
		CGOEnabled: cgoEnabled(),
	}
}

func capturePitRepo(repoRoot string) PitRepo {
	// DirtyKnown stays false unless git successfully reports the status, so a
	// build whose provenance cannot be audited (git unavailable, not a repo,
	// command error) is reported as "unknown", never falsely as "clean".
	p := PitRepo{Commit: unknown}

	commit, err := runGit(repoRoot, "rev-parse", "HEAD")
	if err == nil && commit != "" {
		p.Commit = commit
	}

	status, err := runGit(repoRoot, "status", "--porcelain")
	if err == nil {
		p.Dirty = strings.TrimSpace(status) != ""
		p.DirtyKnown = true
	}

	return p
}

// runGit runs a git command rooted at dir and returns trimmed stdout.
func runGit(dir string, args ...string) (string, error) {
	cmd := exec.Command("git", args...) //nolint:gosec // G204: fixed diagnostic command
	cmd.Dir = dir
	out, err := cmd.Output()
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(out)), nil
}

func captureCore() CoreBuildProfile {
	raw := openpit.GetBuildProfile()
	p := CoreBuildProfile{
		Raw:     raw,
		Version: openpit.GetVersion(),
	}
	p.Profile = unknown
	p.OptLevel = unknown
	p.Target = unknown
	p.TargetCPU = unknown
	p.LTO = unknown

	// Parse key=value; pairs. The format is stable and documented in engine_builder.go.
	for _, pair := range strings.Split(raw, ";") {
		pair = strings.TrimSpace(pair)
		if pair == "" {
			continue
		}
		k, v, ok := strings.Cut(pair, "=")
		if !ok {
			continue
		}
		k = strings.TrimSpace(k)
		v = strings.TrimSpace(v)
		switch k {
		case "profile":
			p.Profile = v
		case "opt_level":
			p.OptLevel = v
		case "debug_assertions":
			p.DebugAssertions = v == "true"
		case "target":
			p.Target = v
		case "target_cpu":
			p.TargetCPU = v
		case "lto":
			p.LTO = v
		case "version":
			// version is already in p.Version from GetVersion(); skip
		}
	}

	return p
}
