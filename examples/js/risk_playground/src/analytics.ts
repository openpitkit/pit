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

import posthog from "posthog-js";

type EventProperties = Readonly<Record<string, boolean | string>>;

// Only the deployed page reports; dev servers, previews, and forks stay silent.
const ANALYTICS_HOST = "openpit.dev";

let initialized = false;

/**
 * Starts PostHog EU analytics with session replay and masked form inputs on
 * the deployed page.
 */
export function initializeAnalytics(): void {
  if (initialized || window.location.hostname !== ANALYTICS_HOST) {
    return;
  }

  posthog.init("phc_q3EM7XmhYeX4xvaWEyfZFPqFJdPaJiGcbALhSBksidxK", {
    api_host: "https://eu.i.posthog.com",
    autocapture: true,
    capture_exceptions: false,
    capture_pageleave: true,
    capture_pageview: true,
    disable_session_recording: false,
    persistence: "localStorage",
    person_profiles: "always",
    session_recording: { maskAllInputs: true },
  });
  initialized = true;
  trackPlaygroundEvent("risk_playground_opened");
}

/** Adds fixed, non-numeric metadata to a meaningful playground action. */
export function trackPlaygroundEvent(
  event: string,
  properties?: EventProperties,
): void {
  if (!initialized) {
    return;
  }
  posthog.capture(event, properties);
}
