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

// clang-format off

// ruleid: comment-width
// This deliberately long comment has enough words to require another line in this fixture.

// ok: comment-width
/* https://example.com/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa */
// ruleid: comment-width
/* This deliberately long comment has enough words to require another line in this fixture. */

// ruleid: comment-width
/** This deliberately long comment has enough words to require another line in this fixture.
 // ruleid: comment-width-block-continuation
 * This deliberately long comment has enough words to require another line in this fixture.
 // ok: comment-width-block-continuation
 * https://example.com/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
 // ruleid: comment-width-block-continuation
 * This deliberately long comment has enough words to require another line in this fixture.
 */

// ok: comment-width
int value = 1; // This deliberately long comment has enough words to require another line in this fixture.

// ok: comment-width
int other = 1; /* This deliberately long comment has enough words to require another line in this fixture.
 // ruleid: comment-width-block-continuation
 * This deliberately long comment has enough words to require another line in this fixture.
 */

void assign(int* slot, int* answer) {
    // ok: comment-width-block-continuation
    *slot = 1000000000 + 1000000000 - 1000000000 + 1000000000 - 1000000000 + 1000000000;
    // ok: comment-width-block-continuation
    *answer == 1000000000 + 1000000000 - 1000000000 + 1000000000 - 1000000000 + 1000000000;
    // ok: comment-width-block-continuation
    * slot = 1000000000 + 1000000000 - 1000000000 + 1000000000 - 1000000000 + 1000000000;
    const auto product = value
        // ok: comment-width-block-continuation
        * (1000000000 + 1000000000 - 1000000000 + 1000000000 - 1000000000 + 1000000000);
}

// clang-format on
