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

// ruleid: short-generic-parameter-js
function single<T>(value: T): T { return value; }
// ruleid: short-generic-parameter-js
class Digit<T1> {}
// ruleid: short-generic-parameter-js
interface Acronym<LPF> {}
// ruleid: short-generic-parameter-js
type ShortName<Id> = { id: Id };
// ruleid: short-generic-parameter-js
function literal<const N extends number>(value: N): N { return value; }
// ruleid: short-generic-parameter-js
function nested<
    Container extends Iterable<Array<Value>>,
    Ab,
    Value,
>(value: Ab): Ab { return value; }
class Methods {
    // ruleid: short-generic-parameter-js
    method<T>() {}
}
interface Signatures {
    // ruleid: short-generic-parameter-js
    method<T>(value: T): T;
}
// ruleid: short-generic-parameter-js
const arrow = <T,>(value: T): T => value;
// ruleid: short-generic-parameter-js
consume<Value>({ method<T>() {} }).done();
// ruleid: short-generic-parameter-js
promise.then(<T,>(x: T) => x).catch(handle);
// ruleid: short-generic-parameter-js
(<T,>(value: T) => value)(1);
// ruleid: short-generic-parameter-js
fn as (<T>(value: T) => T);
// ruleid: short-generic-parameter-js
flag ? <T,>(value: T) => value : other;
// ok: short-generic-parameter-js
function full<Value extends Iterable<Array<T>>>(value: Value): Value {
    return value;
}
// ok: short-generic-parameter-js
const call = single<T>(value);
// todook: short-generic-parameter-js
const conditional = flag ? single<T>(value) : fallback;
// todook: short-generic-parameter-js
const constructed = flag ? new Digit<T>() : fallback;
// todook: short-generic-parameter-js
const asserted = flag ? <T>(value) : () => fallback;
switch (value) {
    // todook: short-generic-parameter-js
    case fn<T>(x):
        break;
}
// ok: short-generic-parameter-js
type Concrete = Wrapper<T>;
// ok: short-generic-parameter-js
const choice = a < MAX ? b : c;
// ok: short-generic-parameter-js
if (x < N && y > M) {}
