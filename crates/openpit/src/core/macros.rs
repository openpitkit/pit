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
// Please see https://github.com/openpitkit and the OWNERS file for details.

#[macro_export]
macro_rules! impl_request_has_field {
    (
        $trait:ident,
        $method:ident,
        &$ret:ty, $type:ty,
        $field:ident,
        $wrapper:ident,
        $wrapper_field:ident,
    ) => {
        impl $trait for $type {
            fn $method(&self) -> &$ret {
                &self.$field
            }
        }

        impl<T> $trait for $wrapper<T> {
            fn $method(&self) -> &$ret {
                self.$wrapper_field.$method()
            }
        }
    };

    (
        $trait:ident,
        $method:ident,
        $ret:ty,
        $type:ty,
        $field:ident,
        $wrapper:ident,
        $wrapper_field:ident,
    ) => {
        impl $trait for $type {
            fn $method(&self) -> $ret {
                self.$field.clone()
            }
        }

        impl<T> $trait for $wrapper<T> {
            fn $method(&self) -> $ret {
                self.$wrapper_field.$method()
            }
        }
    };
}
