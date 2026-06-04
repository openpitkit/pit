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
// Please see https://github.com/openpitkit and the OWNERS file for details.

package main

import (
	"fmt"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
)

const (
	bytesPerKiB = 1024
	bytesPerGiB = 1024 * 1024 * 1024
)

// platformInfo summarizes the host the scenario ran on. Detection is
// best-effort: a field that cannot be determined stays "unknown" rather than
// failing the run.
type platformInfo struct {
	Hardware string
	CPU      string
	Cores    int
	Memory   string
	Disk     string
	OS       string
	Arch     string
	Go       string
}

func gatherPlatform() platformInfo {
	p := platformInfo{
		Hardware: "unknown",
		CPU:      "unknown",
		Cores:    runtime.NumCPU(),
		Memory:   "unknown",
		Disk:     "unknown",
		OS:       "unknown",
		Arch:     runtime.GOOS + "/" + runtime.GOARCH,
		Go:       runtime.Version(),
	}
	switch runtime.GOOS {
	case "darwin":
		gatherDarwin(&p)
	case "linux":
		gatherLinux(&p)
	}
	return p
}

// printPlatform writes the host summary that heads each run's report.
func printPlatform() {
	p := gatherPlatform()
	fmt.Println("Platform:")
	fmt.Printf("  hardware : %s\n", p.Hardware)
	fmt.Printf("  cpu      : %s (%d cores)\n", p.CPU, p.Cores)
	fmt.Printf("  memory   : %s\n", p.Memory)
	fmt.Printf("  disk     : %s\n", p.Disk)
	fmt.Printf("  os       : %s (%s)\n", p.OS, p.Arch)
	fmt.Printf("  go       : %s\n", p.Go)
	fmt.Println()
}

func gatherDarwin(p *platformInfo) {
	if v := runOut("sysctl", "-n", "hw.model"); v != "" {
		p.Hardware = v
	}
	if v := runOut("sysctl", "-n", "machdep.cpu.brand_string"); v != "" {
		p.CPU = v
	}
	if v := runOut("sysctl", "-n", "hw.memsize"); v != "" {
		if n, err := strconv.ParseUint(v, 10, 64); err == nil {
			p.Memory = formatGiB(n)
		}
	}
	name := runOut("sw_vers", "-productName")
	version := runOut("sw_vers", "-productVersion")
	if combined := strings.TrimSpace(name + " " + version); combined != "" {
		p.OS = combined
	}
	if v := darwinDiskInterface(); v != "" {
		p.Disk = v
	}
}

// darwinDiskInterface reports the boot volume's transport (for example "Apple
// Fabric" or "PCI-Express") from diskutil.
func darwinDiskInterface() string {
	for _, line := range strings.Split(runOut("diskutil", "info", "/"), "\n") {
		if rest, ok := strings.CutPrefix(strings.TrimSpace(line), "Protocol:"); ok {
			return strings.TrimSpace(rest)
		}
	}
	return ""
}

func gatherLinux(p *platformInfo) {
	if v := linuxOSPretty(); v != "" {
		p.OS = v
	}
	if v := linuxCPUModel(); v != "" {
		p.CPU = v
	}
	if v := linuxMemTotal(); v != "" {
		p.Memory = v
	}
	if v := readFirstLine("/sys/class/dmi/id/product_name"); v != "" {
		p.Hardware = v
	}
	if v := linuxDiskInterface(); v != "" {
		p.Disk = v
	}
}

func linuxOSPretty() string {
	for _, line := range readLines("/etc/os-release") {
		if rest, ok := strings.CutPrefix(line, "PRETTY_NAME="); ok {
			return strings.Trim(rest, `"`)
		}
	}
	return ""
}

func linuxCPUModel() string {
	for _, line := range readLines("/proc/cpuinfo") {
		key, val, ok := strings.Cut(line, ":")
		if !ok {
			continue
		}
		switch strings.TrimSpace(key) {
		case "model name", "Model", "Hardware":
			return strings.TrimSpace(val)
		}
	}
	return ""
}

func linuxMemTotal() string {
	for _, line := range readLines("/proc/meminfo") {
		rest, ok := strings.CutPrefix(line, "MemTotal:")
		if !ok {
			continue
		}
		fields := strings.Fields(rest) // "<kB> kB"
		if len(fields) == 0 {
			return ""
		}
		if kb, err := strconv.ParseUint(fields[0], 10, 64); err == nil {
			return formatGiB(kb * bytesPerKiB)
		}
	}
	return ""
}

// linuxDiskInterface reports the transport (nvme/sata/usb/...) of the disk
// backing the root filesystem.
func linuxDiskInterface() string {
	src := runOut("findmnt", "-no", "SOURCE", "/")
	if src == "" {
		return ""
	}
	for _, line := range strings.Split(runOut("lsblk", "-no", "TRAN", src), "\n") {
		if t := strings.TrimSpace(line); t != "" {
			return t
		}
	}
	return ""
}

func formatGiB(b uint64) string {
	return fmt.Sprintf("%.1f GiB", float64(b)/bytesPerGiB)
}

// runOut runs a fixed diagnostic command and returns trimmed stdout, or "" on
// any error. The command and arguments are constants, never user input.
func runOut(name string, args ...string) string {
	out, err := exec.Command(name, args...).Output() //nolint:gosec // G204: fixed diagnostic commands, no user input
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(out))
}

func readLines(path string) []string {
	data, err := os.ReadFile(path) //nolint:gosec // G304: fixed system-info paths, no user input
	if err != nil {
		return nil
	}
	return strings.Split(string(data), "\n")
}

func readFirstLine(path string) string {
	lines := readLines(path)
	if len(lines) == 0 {
		return ""
	}
	return strings.TrimSpace(lines[0])
}
