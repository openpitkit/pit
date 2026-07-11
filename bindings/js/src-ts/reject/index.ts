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

/**
 * `@openpit/engine/reject` - business rejects and account-block types.
 *
 * Exposes the `Reject` value returned on a pipeline result, the stable
 * {@link RejectCode} and {@link RejectScope} vocabularies, the out-of-band
 * `AccountBlock`, and the `AccountControl` handle a policy uses to block
 * accounts from a callback.
 *
 * A reject is NOT an exception: `engine.startPreTrade()` and `request.execute()`
 * return rejects on the result object. The {@link AccountBlockError} thrown by
 * the admin block operations is re-exported here for convenience; it is the
 * same class as on the root `@openpit/engine` error surface.
 *
 * @packageDocumentation
 */

// Importing any subpath initializes its platform runtime; package wrappers keep
// one module graph when callers mix public entries.
import "#runtime";

export {
  // Reject + block + account-control handle.
  Reject,
  AccountBlock,
  AccountControl,
} from "../wasm/openpit_js.js";

// String-discriminated reject vocabularies: value sets plus derived union
// types.
export { RejectCode, RejectScope } from "../types.js";

// The admin-block error and its discriminant. Canonically part of the typed
// error hierarchy, re-exported here as reject-domain types (same class
// identity as on the root entry).
export { AccountBlockError, AccountBlockErrorKind } from "../errors.js";
