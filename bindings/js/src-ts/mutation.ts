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

/**
 * A synchronous side effect invoked with `this` bound to its mutation object.
 *
 * The engine binds `this` to whichever object carries the pair - the
 * {@link Mutation} instance, or the plain `{ commit, rollback }` literal a
 * policy returned - so a callback cannot know more about `this` than that it is
 * an object, and its members read as `unknown`. A callback that needs typed
 * state should close over that state instead of reaching through `this`.
 */
export type MutationFn = (
  this: Record<PropertyKey, unknown>,
) => SynchronousMutationResult;

/**
 * A commit/rollback pair returned from a custom policy decision.
 *
 * Apply tentative state before registering the pair. For an ordinary
 * reservation, the engine calls `commit()` after the venue acknowledges the
 * order, or `rollback()` otherwise. A drop-copy operation registers the same
 * pair and finalizes it the same way, from `commit()` or `rollback()` on the
 * `DropCopyOperation`. A rollback also runs for mutations whose commit was
 * never reached, because their tentative state was already applied.
 *
 * A policy may register this pair by returning it from `performPreTradeCheck`,
 * or from `checkPreTradeStart` through
 * `ctx.recordDropCopyStartMutation(mutation)` for drop-copy-only start work.
 *
 * Neither callback has the right to fail. By the time a finalizer runs the
 * decision is already made and the state it finalizes was applied eagerly, so
 * there is nothing left to compensate. A callback that throws anyway is
 * reported on two independent channels. The throw itself reaches whoever called
 * `commit()` / `rollback()`, wrapped in a `PolicyCallbackError`, once every
 * remaining callback of the batch has run - a failing callback never stops the
 * batch. Separately, the engine arms its kill switch, because its own
 * bookkeeping is now in an unknown state. Every mutation registered from
 * JavaScript belongs to a custom policy, whose state reach the engine cannot
 * bound, so that kill switch blocks **every** account, not only the order's
 * own. Nothing reports the block to the finalizing caller: it surfaces when the
 * next pre-trade call is rejected with `SystemUnavailable`. An operator clears
 * it with `engine.accounts().unblockAll()`.
 *
 * Both callbacks receive this instance as `this`; see {@link MutationFn} for
 * what that binding does and does not promise. There is no wasm class for it
 * because it carries JS closures the engine invokes.
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
