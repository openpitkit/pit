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

// ruleid: short-generic-parameter-cpp
template <typename T> struct Single {};
// ruleid: short-generic-parameter-cpp
template <class T1> struct Digit {};
// ruleid: short-generic-parameter-cpp
template <typename... LPF> struct Acronym {};
// ruleid: short-generic-parameter-cpp
template <typename Op> struct ShortName {};
// ruleid: short-generic-parameter-cpp
template <unsigned int N> struct FixedBuffer {};
// ruleid: short-generic-parameter-cpp
template <typename Container = Wrapper<List<Value>>, class Ab = Value>
struct Nested {};
// ok: short-generic-parameter-cpp
template <typename Value, int Size> struct Full {};
// ok: short-generic-parameter-cpp
template <typename Value = Wrapper<T>> struct NestedReference {};
// ok: short-generic-parameter-cpp
using Concrete = Wrapper<T>;
