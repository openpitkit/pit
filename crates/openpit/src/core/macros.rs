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

/// Generates `Has*` request-field trait implementations for a base payload type and its wrapper.
///
/// This macro is intended for SDK extension points where wrapper composition must preserve access
/// to request fields through nested `With*` types.
///
/// Supported form:
/// ```rust
/// use openpit::{HasInstrument, Instrument, impl_request_has_field};
/// use openpit::param::Asset;
///
/// #[derive(Clone)]
/// struct BaseOrder {
///     instrument: Instrument,
/// }
///
/// struct WithOrder<T> {
///     inner: T,
///     order: BaseOrder,
/// }
///
/// impl_request_has_field!(
///     BaseOrder,
///     WithOrder,
///     order,
///     HasInstrument, instrument, &Instrument, instrument;
/// );
///
/// let instrument = Instrument::new(
///     Asset::new("BTC").expect("must be valid"),
///     Asset::new("USD").expect("must be valid"),
/// );
/// let wrapper = WithOrder {
///     inner: (),
///     order: BaseOrder {
///         instrument: instrument.clone(),
///     },
/// };
///
/// assert_eq!(wrapper.instrument(), Ok(&instrument));
/// ```
///
/// Behavior:
/// - Implements each listed `Has*` trait for the base type and the wrapper.
/// - Wrapper methods delegate to `self.<wrapper_field>.<method>()`.
/// - All generated methods return `Result<_, RequestFieldAccessError>`.
/// - Owned/copy return form clones the base field with `self.<field>.clone()`.
/// - Reference return form (`&T`) borrows the base field as `&self.<field>`.
///
/// Notes:
/// - For fields that require custom conversion (for example `Option<Asset>` -> `Option<&Asset>`),
///   provide a manual impl instead of this macro.
#[macro_export]
macro_rules! impl_request_has_field {
    (
        $type:ty,
        $wrapper:ident,
        $wrapper_field:ident,
        $(
            $trait:ident, $method:ident, &$ret:ty, $field:ident;
        )+
    ) => {
        $(
            impl $trait for $type {
                fn $method(
                    &self,
                ) -> ::std::result::Result<&$ret, $crate::RequestFieldAccessError> {
                    Ok(&self.$field)
                }
            }

            impl<T> $trait for $wrapper<T> {
                fn $method(
                    &self,
                ) -> ::std::result::Result<&$ret, $crate::RequestFieldAccessError> {
                    self.$wrapper_field.$method()
                }
            }
        )+
    };

    (
        $type:ty,
        $wrapper:ident,
        $wrapper_field:ident,
        $(
            $trait:ident, $method:ident, $ret:ty, $field:ident;
        )+
    ) => {
        $(
            impl $trait for $type {
                fn $method(
                    &self,
                ) -> ::std::result::Result<$ret, $crate::RequestFieldAccessError> {
                    Ok(self.$field.clone())
                }
            }

            impl<T> $trait for $wrapper<T> {
                fn $method(
                    &self,
                ) -> ::std::result::Result<$ret, $crate::RequestFieldAccessError> {
                    self.$wrapper_field.$method()
                }
            }
        )+
    };
}

/// Generates passthrough `Has*` implementations for a wrapper through its `inner`-like field.
///
/// Use this when a wrapper adds its own fields but must keep all previously available `Has*`
/// capabilities from the composed inner type.
///
/// Supported form:
/// ```rust
/// use openpit::{
///     HasInstrument, Instrument, RequestFieldAccessError, impl_request_has_field_passthrough,
/// };
/// use openpit::param::Asset;
///
/// struct Inner {
///     instrument: Instrument,
/// }
///
/// impl HasInstrument for Inner {
///     fn instrument(&self) -> Result<&Instrument, RequestFieldAccessError> {
///         Ok(&self.instrument)
///     }
/// }
///
/// struct Wrapper<T> {
///     inner: T,
/// }
///
/// impl_request_has_field_passthrough!(
///     Wrapper,
///     inner,
///     HasInstrument, instrument, &Instrument;
/// );
///
/// let instrument = Instrument::new(
///     Asset::new("ETH").expect("must be valid"),
///     Asset::new("USD").expect("must be valid"),
/// );
/// let wrapper = Wrapper {
///     inner: Inner {
///         instrument: instrument.clone(),
///     },
/// };
///
/// assert_eq!(wrapper.instrument(), Ok(&instrument));
/// ```
///
/// Behavior:
/// - Generates `impl<T> Trait for Wrapper<T> where T: Trait`.
/// - Each method delegates to `self.<inner_field>.<method>()`.
/// - All generated methods return `Result<_, RequestFieldAccessError>`.
#[macro_export]
macro_rules! impl_request_has_field_passthrough {
    (
        $wrapper:ident,
        $inner_field:ident,
        $(
            $trait:ident, $method:ident, &$ret:ty;
        )+
    ) => {
        $(
            impl<T> $trait for $wrapper<T>
            where
                T: $trait,
            {
                fn $method(
                    &self,
                ) -> ::std::result::Result<&$ret, $crate::RequestFieldAccessError> {
                    self.$inner_field.$method()
                }
            }
        )+
    };

    (
        $wrapper:ident,
        $inner_field:ident,
        $(
            $trait:ident, $method:ident, $ret:ty;
        )+
    ) => {
        $(
            impl<T> $trait for $wrapper<T>
            where
                T: $trait,
            {
                fn $method(
                    &self,
                ) -> ::std::result::Result<$ret, $crate::RequestFieldAccessError> {
                    self.$inner_field.$method()
                }
            }
        )+
    };
}

#[cfg(test)]
mod tests {
    use crate::RequestFieldAccessError;

    trait HasNameRef {
        fn name_ref(&self) -> Result<&str, RequestFieldAccessError>;
    }

    trait HasCount {
        fn count(&self) -> Result<u32, RequestFieldAccessError>;
    }

    #[derive(Clone)]
    struct Base {
        name: String,
        count: u32,
    }

    struct WithBase<T> {
        #[allow(dead_code)]
        inner: T,
        base: Base,
    }

    crate::impl_request_has_field!(
        Base,
        WithBase,
        base,
        HasNameRef, name_ref, &str, name;
    );

    crate::impl_request_has_field!(
        Base,
        WithBase,
        base,
        HasCount, count, u32, count;
    );

    trait HasInnerNameRef {
        fn inner_name_ref(&self) -> Result<&str, RequestFieldAccessError>;
    }

    trait HasInnerCount {
        fn inner_count(&self) -> Result<u32, RequestFieldAccessError>;
    }

    struct InnerCaps {
        name: String,
        count: u32,
    }

    impl HasInnerNameRef for InnerCaps {
        fn inner_name_ref(&self) -> Result<&str, RequestFieldAccessError> {
            Ok(self.name.as_str())
        }
    }

    impl HasInnerCount for InnerCaps {
        fn inner_count(&self) -> Result<u32, RequestFieldAccessError> {
            Ok(self.count)
        }
    }

    struct Wrapper<T> {
        inner: T,
    }

    crate::impl_request_has_field_passthrough!(
        Wrapper,
        inner,
        HasInnerNameRef, inner_name_ref, &str;
    );

    crate::impl_request_has_field_passthrough!(
        Wrapper,
        inner,
        HasInnerCount, inner_count, u32;
    );

    #[test]
    fn impl_request_has_field_generates_ref_and_owned_accessors() {
        let base = Base {
            name: "alpha".to_string(),
            count: 7,
        };
        assert_eq!(base.name_ref(), Ok("alpha"));
        assert_eq!(base.count(), Ok(7));

        let wrapped = WithBase {
            inner: (),
            base: base.clone(),
        };
        assert_eq!(wrapped.name_ref(), Ok("alpha"));
        assert_eq!(wrapped.count(), Ok(7));
    }

    #[test]
    fn impl_request_has_field_passthrough_delegates_for_ref_and_owned_forms() {
        let wrapped = Wrapper {
            inner: InnerCaps {
                name: "beta".to_string(),
                count: 11,
            },
        };
        assert_eq!(wrapped.inner_name_ref(), Ok("beta"));
        assert_eq!(wrapped.inner_count(), Ok(11));
    }
}
