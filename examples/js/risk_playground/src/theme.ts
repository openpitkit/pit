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

export type ThemeMode = "dark" | "light" | "system";
export type ResolvedTheme = Exclude<ThemeMode, "system">;

const THEME_STORAGE_KEY = "openpit-playground-theme";

export function readTheme(): ThemeMode {
  try {
    const value = window.localStorage.getItem(THEME_STORAGE_KEY);
    return isThemeMode(value) ? value : "system";
  } catch (error) {
    console.warn("theme preference unreadable, using the system theme", error);
    // fallback(approved): blocked storage must not break the page; the theme is a convenience
    return "system";
  }
}

export function saveTheme(theme: ThemeMode): void {
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, theme);
  } catch (error) {
    // fallback(approved): blocked storage keeps the selected theme for this visit only
    console.warn("theme preference not saved", error);
  }
}

export function resolveTheme(theme: ThemeMode): ResolvedTheme {
  if (theme !== "system") {
    return theme;
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function applyTheme(theme: ThemeMode): ResolvedTheme {
  const resolved = resolveTheme(theme);
  const root = document.documentElement;
  root.classList.toggle("dark", resolved === "dark");
  root.classList.toggle("light", resolved === "light");
  root.style.colorScheme = resolved;
  document
    .querySelector<HTMLLinkElement>("link[data-theme-favicon]")
    ?.setAttribute("href", `./favicon-${resolved}.svg`);
  document
    .querySelector<HTMLMetaElement>("meta[data-theme-color]")
    ?.setAttribute("content", resolved === "dark" ? "#0a0d12" : "#f6f3ec");
  return resolved;
}

function isThemeMode(value: string | null): value is ThemeMode {
  return value === "dark" || value === "light" || value === "system";
}
