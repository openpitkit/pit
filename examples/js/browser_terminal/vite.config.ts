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

import { defineConfig } from "vite";

// A bundler (Vite) resolves "@openpit/engine" to its browser entry, which has
// the wasm base64-inlined into the JS. That is the whole point of this demo:
// the production build is a static bundle with no sidecar .wasm to fetch and no
// server to talk to. `assetsInlineLimit: 0` is intentionally NOT set - we rely
// on the package's own inlined-wasm browser build, not on Vite asset inlining.
export default defineConfig({
  // Relative base so the built dist/ can be opened from any sub-path or a CDN.
  base: "./",
  build: {
    target: "es2022",
    // Surface the bundle size so the inlined-wasm footprint is visible in the
    // build summary instead of being warned about and hidden.
    chunkSizeWarningLimit: 4096,
  },
});
