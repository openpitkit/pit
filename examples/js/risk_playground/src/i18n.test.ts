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

import {
  CATALOGS,
  LOCALES,
  detectLocale,
  matchLocale,
  messageParts,
  resolveLocale,
  translate,
  withLocale,
  type LocaleId,
  type MessageKey,
} from "./i18n.ts";

const LOCALE_IDS = LOCALES.map(({ id }) => id);
const KEYS = Object.keys(CATALOGS.en).sort() as MessageKey[];

function placeholders(locale: LocaleId, key: MessageKey): string[] {
  return messageParts(locale, key)
    .filter((_, index) => index % 2 === 1)
    .sort();
}

describe("catalogs", () => {
  it("cover the languages of openpit.dev", () => {
    expect(LOCALE_IDS).toEqual(["en", "de", "es", "ru", "uk", "zh-CN", "hi"]);
  });

  it.each(LOCALE_IDS)("%s has exactly the English keys", (locale) => {
    expect(Object.keys(CATALOGS[locale]).sort()).toEqual(KEYS);
  });

  it.each(LOCALE_IDS)("%s has no empty message", (locale) => {
    for (const key of KEYS) {
      expect(CATALOGS[locale][key].trim(), `${locale}: ${key}`).not.toBe("");
    }
  });

  it.each(LOCALE_IDS)("%s keeps the English placeholders", (locale) => {
    for (const key of KEYS) {
      expect(placeholders(locale, key), `${locale}: ${key}`).toEqual(
        placeholders("en", key),
      );
    }
  });
});

describe("matchLocale", () => {
  it("matches the exact tag regardless of case", () => {
    expect(matchLocale("zh-CN")).toBe("zh-CN");
    expect(matchLocale("ZH-cn")).toBe("zh-CN");
    expect(matchLocale("hi")).toBe("hi");
  });

  it("falls back to the primary language of a regional tag", () => {
    expect(matchLocale("en-GB")).toBe("en");
    expect(matchLocale("de-AT")).toBe("de");
    expect(matchLocale("es-419")).toBe("es");
    expect(matchLocale("uk-UA")).toBe("uk");
    expect(matchLocale("zh-TW")).toBe("zh-CN");
    expect(matchLocale("zh")).toBe("zh-CN");
  });

  it("rejects an unsupported language", () => {
    expect(matchLocale("fr-FR")).toBeNull();
    expect(matchLocale("")).toBeNull();
  });
});

describe("detectLocale", () => {
  it("takes the first supported language in the browser's order", () => {
    expect(detectLocale(["fr-FR", "de-DE", "en"])).toBe("de");
    expect(detectLocale(["ru-RU", "en-US"])).toBe("ru");
  });

  it("keeps English when it outranks a supported language", () => {
    expect(detectLocale(["en-US", "ru"])).toBe("en");
  });

  it("defaults to English when no language is supported", () => {
    expect(detectLocale(["fr", "ja-JP"])).toBe("en");
    expect(detectLocale([])).toBe("en");
  });
});

describe("resolveLocale", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("lets the lang parameter override the browser", () => {
    expect(resolveLocale("?lang=ru", ["de"])).toBe("ru");
    expect(resolveLocale("?a=1&lang=zh", ["en"])).toBe("zh-CN");
  });

  it("detects from the browser without a lang parameter", () => {
    expect(resolveLocale("", ["uk-UA"])).toBe("uk");
    expect(resolveLocale("?a=1", ["fr"])).toBe("en");
  });

  it("warns about and ignores an unsupported lang value", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    expect(resolveLocale("?lang=fr", ["uk"])).toBe("uk");
    expect(warn).toHaveBeenCalledOnce();
  });
});

describe("withLocale", () => {
  it("sets the lang parameter and keeps the rest of the address", () => {
    expect(
      withLocale("https://openpit.dev/playground/?a=1#your-limits", "ru"),
    ).toBe("https://openpit.dev/playground/?a=1&lang=ru#your-limits");
    expect(withLocale("https://openpit.dev/playground/?lang=de", "hi")).toBe(
      "https://openpit.dev/playground/?lang=hi",
    );
  });
});

describe("translate", () => {
  it("fills placeholders with text and numbers", () => {
    expect(translate("en", "order.fee", { fee: "$0.25 USD" })).toBe(
      "Fixed venue fee - $0.25 USD",
    );
    expect(translate("en", "limit.invalidMax", { max: 60n })).toBe(
      "Enter a positive whole number no greater than 60.",
    );
    expect(translate("ru", "ribbon.rateValue", { rate: 5 })).toBe("5 / мин");
  });

  it("alternates text and placeholder names when split", () => {
    expect(messageParts("en", "order.mark")).toEqual([
      "simulated mark ",
      "mark",
      "",
    ]);
  });

  it("fails when a placeholder has no value", () => {
    expect(() => translate("en", "order.fee")).toThrow(/order\.fee.*fee/);
  });
});
