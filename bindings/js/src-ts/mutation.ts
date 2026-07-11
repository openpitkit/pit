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
 * A callback result that is known to complete synchronously.
 *
 * The engine ignores this value. Primitive and non-thenable object returns are
 * accepted so idiomatic expression-bodied callbacks remain valid; Promise and
 * thenable returns are rejected by both TypeScript and the runtime boundary.
 */
export type SynchronousMutationResult =
  | void
  | null
  | string
  | number
  | boolean
  | bigint
  | symbol
  | (object & { readonly then?: never });

/** A synchronous side effect that can be committed or rolled back. */
export type MutationFn = () => SynchronousMutationResult;

/**
 * A commit/rollback pair returned from a custom policy decision.
 *
 * The engine calls `commit()` when the surrounding pre-trade transaction is
 * accepted and the downstream venue acknowledged the order, or `rollback()`
 * otherwise. Exactly one of the two runs, exactly once. There is no wasm class
 * for it because it carries JS closures the engine invokes.
 */
export class Mutation {
  /** Applies the side effect. */
  readonly commit: MutationFn;
  /** Reverts the side effect. */
  readonly rollback: MutationFn;

  constructor(commit: MutationFn, rollback: MutationFn) {
    this.commit = commit;
    this.rollback = rollback;
  }
}
