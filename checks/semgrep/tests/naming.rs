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

// ruleid: short-generic-parameter-rust
fn single<T>() {}
// ruleid: short-generic-parameter-rust
impl<T> Wrapper<T> {}
// ruleid: short-generic-parameter-rust
struct Digit<T1>(T1);
// ruleid: short-generic-parameter-rust
enum Acronym<LPF> {
    Item(LPF),
}
// ruleid: short-generic-parameter-rust
type ShortName<Id> = Option<Id>;
// ruleid: short-generic-parameter-rust
trait FixedBuffer<const N: usize> {}
// ruleid: short-generic-parameter-rust
fn nested<Container: IntoIterator<Item = Vec<Option<Value>>>, Ab: Clone, Value>() {}
// ok: short-generic-parameter-rust
fn full<'a, Value, const Size: usize>(_: &'a [Value; Size]) {}
// ok: short-generic-parameter-rust
impl<'a> Borrowed<'a> {}
// ok: short-generic-parameter-rust
fn nested_reference<Value: Iterator<Item = T>>() {}
// ok: short-generic-parameter-rust
fn callback_reference<Callback: Fn(First, T, Last)>() {}
// ok: short-generic-parameter-rust
fn with_lifetime<'AB>() {}
