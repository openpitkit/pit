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

import {
  createContext,
  Fragment,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

import type { ReactNode } from "react";

import {
  messageParts,
  resolveLocale,
  translate,
  withLocale,
  type LocaleId,
  type MessageKey,
  type Translate,
} from "./i18n.ts";

/** Elements substituted for the `{{name}}` placeholders of a message. */
export type MessageSlots = Readonly<Record<string, ReactNode>>;

/** The active language and what renders text in it. */
export interface I18n {
  readonly locale: LocaleId;
  /** Switches the language and pins it in the page address. */
  readonly setLocale: (locale: LocaleId) => void;
  readonly t: Translate;
  /** Renders a message whose placeholders stand for elements, not text. */
  readonly rich: (key: MessageKey, slots: MessageSlots) => ReactNode;
}

const I18nContext = createContext<I18n | null>(null);

/**
 * Provides the playground language, chosen once from the `lang` parameter and
 * the browser's preferences, and keeps the document language and title in step.
 */
export function I18nProvider({ children }: { readonly children: ReactNode }) {
  const [locale, setLocale] = useState<LocaleId>(() =>
    resolveLocale(window.location.search, window.navigator.languages),
  );

  useEffect(() => {
    document.documentElement.lang = locale;
    document.title = translate(locale, "meta.title");
  }, [locale]);

  const value = useMemo<I18n>(
    () => ({
      locale,
      setLocale: (next) => {
        window.history.replaceState(
          null,
          "",
          withLocale(window.location.href, next),
        );
        setLocale(next);
      },
      t: (key, params) => translate(locale, key, params),
      rich: (key, slots) =>
        messageParts(locale, key).map((part, index) => {
          if (index % 2 === 0) {
            return part;
          }
          if (!Object.hasOwn(slots, part)) {
            throw new Error(`message ${key} needs an element for {{${part}}}`);
          }
          return <Fragment key={index}>{slots[part]}</Fragment>;
        }),
    }),
    [locale],
  );

  return <I18nContext value={value}>{children}</I18nContext>;
}

/** Returns the playground language; only valid under {@link I18nProvider}. */
export function useI18n(): I18n {
  const value = useContext(I18nContext);
  if (value === null) {
    throw new Error("useI18n must be used inside I18nProvider");
  }
  return value;
}
