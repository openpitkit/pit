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

import { afterEach, describe, expect, it, vi } from "vitest";

import { readTheme, saveTheme } from "./theme.ts";

function blockedStorage(): Storage {
  const deny = (): never => {
    throw new Error("storage denied");
  };
  return { getItem: deny, setItem: deny } as unknown as Storage;
}

describe("theme storage fallback", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("uses the system theme and warns when storage cannot be read", () => {
    vi.stubGlobal("window", { localStorage: blockedStorage() });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    expect(readTheme()).toBe("system");
    expect(warn).toHaveBeenCalledOnce();
  });

  it("keeps the page running and warns when storage cannot be written", () => {
    vi.stubGlobal("window", { localStorage: blockedStorage() });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    expect(() => saveTheme("dark")).not.toThrow();
    expect(warn).toHaveBeenCalledOnce();
  });
});
