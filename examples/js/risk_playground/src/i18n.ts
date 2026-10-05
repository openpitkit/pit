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

import de from "./locales/de.json";
import en from "./locales/en.json";
import es from "./locales/es.json";
import hi from "./locales/hi.json";
import ru from "./locales/ru.json";
import uk from "./locales/uk.json";
import zhCN from "./locales/zh-CN.json";

/** Keys of the English catalog, which every other catalog mirrors. */
export type MessageKey = keyof typeof en;

/** One locale's messages: every key of the English catalog. */
export type Messages = Readonly<Record<MessageKey, string>>;

/** Values substituted for the `{{name}}` placeholders of a message. */
export type MessageParams = Readonly<Record<string, bigint | number | string>>;

/** Renders a message of the active locale with its placeholders filled. */
export type Translate = (key: MessageKey, params?: MessageParams) => string;

/**
 * The languages of openpit.dev, in its menu order. `name` is the language's
 * own name and is never translated, so a visitor can always find it.
 */
export const LOCALES = [
  { id: "en", name: "English" },
  { id: "de", name: "Deutsch" },
  { id: "es", name: "Español" },
  { id: "ru", name: "Русский" },
  { id: "uk", name: "Українська" },
  { id: "zh-CN", name: "简体中文" },
  { id: "hi", name: "हिन्दी" },
] as const;

/** A language the playground is translated into. */
export type LocaleId = (typeof LOCALES)[number]["id"];

/** English is the source language and the one used when none matches. */
export const DEFAULT_LOCALE: LocaleId = "en";

/** Every catalog by locale; `en` is the source of truth. */
export const CATALOGS: Readonly<Record<LocaleId, Messages>> = {
  de,
  en,
  es,
  hi,
  ru,
  uk,
  "zh-CN": zhCN,
};

const LANGUAGE_PARAM = "lang";

// Splitting on a capturing group alternates literal text (even indexes) with
// placeholder names (odd indexes).
const PLACEHOLDER = /\{\{(\w+)\}\}/;

function primaryLanguage(tag: string): string {
  const dash = tag.indexOf("-");
  return (dash < 0 ? tag : tag.slice(0, dash)).toLowerCase();
}

/**
 * Maps a BCP 47 language tag to a supported locale: the exact tag first, then
 * the first locale with the same primary language, so `en-GB` selects English
 * and every `zh-*` tag selects Simplified Chinese. `null` when unsupported.
 */
export function matchLocale(tag: string): LocaleId | null {
  const wanted = tag.toLowerCase();
  const exact = LOCALES.find(({ id }) => id.toLowerCase() === wanted);
  if (exact !== undefined) {
    return exact.id;
  }
  const primary = primaryLanguage(tag);
  const sameLanguage = LOCALES.find(
    ({ id }) => primaryLanguage(id) === primary,
  );
  return sameLanguage === undefined ? null : sameLanguage.id;
}

/**
 * Picks the first of the visitor's preferred languages, in their order, that
 * the playground supports. English when none is: the default, not an error.
 */
export function detectLocale(languages: readonly string[]): LocaleId {
  for (const language of languages) {
    const locale = matchLocale(language);
    if (locale !== null) {
      return locale;
    }
  }
  return DEFAULT_LOCALE;
}

/**
 * The language to show: the `lang` query parameter when it names a supported
 * language, otherwise the best match for the browser's preferences. An
 * unsupported `lang` value is reported and ignored.
 */
export function resolveLocale(
  search: string,
  languages: readonly string[],
): LocaleId {
  const requested = new URLSearchParams(search).get(LANGUAGE_PARAM);
  if (requested !== null) {
    const locale = matchLocale(requested);
    if (locale !== null) {
      return locale;
    }
    console.warn(
      `unsupported ${LANGUAGE_PARAM} ${JSON.stringify(requested)}, using the browser languages`,
    );
  }
  return detectLocale(languages);
}

/** Returns `href` pinned to one language through its `lang` parameter. */
export function withLocale(href: string, locale: LocaleId): string {
  const url = new URL(href);
  url.searchParams.set(LANGUAGE_PARAM, locale);
  return url.toString();
}

/**
 * Splits a message into alternating literal text and placeholder names: the
 * even entries are text, the odd ones are the names inside `{{...}}`.
 */
export function messageParts(
  locale: LocaleId,
  key: MessageKey,
): readonly string[] {
  return CATALOGS[locale][key].split(PLACEHOLDER);
}

/** Renders a message, throwing when a placeholder has no value. */
export function translate(
  locale: LocaleId,
  key: MessageKey,
  params: MessageParams = {},
): string {
  return messageParts(locale, key)
    .map((part, index) => {
      if (index % 2 === 0) {
        return part;
      }
      const value = params[part];
      if (value === undefined) {
        throw new Error(`message ${key} needs a value for {{${part}}}`);
      }
      return value.toString();
    })
    .join("");
}
