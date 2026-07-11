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
 * `@openpit/engine/tx` - the commit/rollback mutation pair.
 *
 * Exposes the {@link Mutation} a custom policy returns from a decision to
 * register a side effect the engine commits or rolls back with the surrounding
 * pre-trade transaction.
 *
 * This is a JS-only helper (it carries closures the engine invokes); there is
 * no wasm class behind it.
 *
 * @packageDocumentation
 */

// Importing any subpath initializes its platform runtime; package wrappers keep
// one module graph when callers mix public entries.
import "#runtime";

export { Mutation } from "../mutation.js";
export type { MutationFn, SynchronousMutationResult } from "../mutation.js";
