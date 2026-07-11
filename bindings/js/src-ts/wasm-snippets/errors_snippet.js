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
//
// wasm-bindgen snippet imported by the Rust boundary (src/error.rs) via
// `#[wasm_bindgen(module = "/src-ts/wasm-snippets/errors_snippet.js")]`.
//
// wasm-bindgen copies this file verbatim into
// `src-ts/wasm/snippets/<crate-hash>/src-ts/wasm-snippets/errors_snippet.js`
// and rewrites the generated glue to import `makeError` from here, so the
// engine constructs the real `Error` subclasses directly. The relative path
// below resolves up out of that fixed-depth snippet location back to the
// canonical `src-ts/errors` module, giving ONE shared class identity between
// the engine and the public surface (so `instanceof` holds). esbuild dedupes
// the module in the bundled build; native ESM (vitest, Node) resolves it as a
// singleton. Keep this file plain JS - wasm-bindgen does not compile it.
export {
  makeError,
  makeQuoteExpiredError,
} from "../../../../../errors.js";
