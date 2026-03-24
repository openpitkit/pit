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
//! Procedural macros for the `openpit` SDK.
//!
//! This crate provides derive macros that generate request-field capability implementations
//! expected by `openpit` policies.
//!
//! # `RequestFields`
//!
//! Derive for wrapper structs with named fields.
//!
//! Field-level `#[openpit(...)]` items:
//!
//! - `inner`: marks the field used for passthrough delegation.
//! - `TraitPath(method -> ReturnType)`: generate direct impl for the field.
//! - `TraitPath(-> ReturnType)`: same as above, method inferred from `Has*` trait name.
//!
//! On a field marked with `inner`, trait items generate passthrough impls with
//! `where InnerType: TraitPath`.
//!
//! Old syntax `#[request_fields(...)]` is rejected with a compile-time error that points to
//! `#[openpit(...)]`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parenthesized, parse::Parse, parse::ParseStream, parse_macro_input, parse_quote,
    punctuated::Punctuated, Data, DeriveInput, Field, Fields, Generics, Ident, Path, Token, Type,
};

#[proc_macro_derive(RequestFields, attributes(openpit, request_fields))]
pub fn derive_request_fields(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_request_fields_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn derive_request_fields_impl(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = input.ident;
    let generics = input.generics;

    let data = match input.data {
        Data::Struct(data) => data,
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "RequestFields can only be derived for structs",
            ));
        }
    };

    let fields = match data.fields {
        Fields::Named(fields) => fields.named,
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "RequestFields requires named fields",
            ));
        }
    };

    let mut generated = Vec::new();
    let mut seen_traits = std::collections::BTreeSet::new();
    let mut explicit_inner: Option<&Field> = None;

    for field in &fields {
        let Some(field_ident) = &field.ident else {
            continue;
        };

        reject_legacy_request_fields(field)?;

        let parsed = parse_openpit_items(field)?;
        if !parsed.inner {
            for capability in parsed.capabilities {
                register_trait_once(&mut seen_traits, &capability, field)?;
                generated.push(impl_direct_trait(
                    &name,
                    &generics,
                    field_ident,
                    &capability,
                ));
            }
            continue;
        }

        if explicit_inner.is_some() {
            return Err(syn::Error::new_spanned(
                field,
                "only one #[openpit(inner)] field is allowed",
            ));
        }
        explicit_inner = Some(field);

        for capability in parsed.capabilities {
            register_trait_once(&mut seen_traits, &capability, field)?;
            generated.push(impl_passthrough_trait(
                &name,
                &generics,
                field_ident,
                &field.ty,
                &capability,
            ));
        }
    }

    Ok(quote! {
        #(#generated)*
    })
}

fn register_trait_once(
    seen_traits: &mut std::collections::BTreeSet<String>,
    capability: &CapabilitySpec,
    span: &impl quote::ToTokens,
) -> syn::Result<()> {
    let key = quote!(#capability).to_string();
    if !seen_traits.insert(key.clone()) {
        return Err(syn::Error::new_spanned(
            span,
            format!("duplicate trait mapping for {key}"),
        ));
    }
    Ok(())
}

fn reject_legacy_request_fields(field: &Field) -> syn::Result<()> {
    for attr in &field.attrs {
        if attr.path().is_ident("request_fields") {
            return Err(syn::Error::new_spanned(
                attr,
                "legacy #[request_fields(...)] is not supported; use #[openpit(...)]",
            ));
        }
    }
    Ok(())
}

fn parse_openpit_items(field: &Field) -> syn::Result<FieldOpenpitItems> {
    let mut result = FieldOpenpitItems {
        inner: false,
        capabilities: Vec::new(),
    };

    for attr in &field.attrs {
        if !attr.path().is_ident("openpit") {
            continue;
        }

        let items =
            attr.parse_args_with(Punctuated::<OpenpitAttrItem, Token![,]>::parse_terminated)?;
        if items.is_empty() {
            return Err(syn::Error::new_spanned(
                attr,
                "empty #[openpit(...)] is not allowed",
            ));
        }

        for item in items {
            match item {
                OpenpitAttrItem::Inner(span) => {
                    if result.inner {
                        return Err(syn::Error::new_spanned(
                            span,
                            "duplicate `inner` marker in #[openpit(...)]",
                        ));
                    }
                    result.inner = true;
                }
                OpenpitAttrItem::Capability(spec) => result.capabilities.push(*spec),
            }
        }
    }

    Ok(result)
}

struct FieldOpenpitItems {
    inner: bool,
    capabilities: Vec<CapabilitySpec>,
}

enum OpenpitAttrItem {
    Inner(Ident),
    Capability(Box<CapabilitySpec>),
}

impl Parse for OpenpitAttrItem {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let path = input.parse::<Path>()?;
        if path.is_ident("inner") {
            if !input.is_empty() && !input.peek(Token![,]) {
                return Err(input.error("`inner` must not have arguments"));
            }
            let ident = path
                .get_ident()
                .expect("inner path must have one segment")
                .clone();
            return Ok(OpenpitAttrItem::Inner(ident));
        }

        if !input.peek(syn::token::Paren) {
            return Err(syn::Error::new_spanned(
                path,
                "invalid #[openpit(...)] item; expected `Trait(method -> ReturnType)` or `Trait(-> ReturnType)`",
            ));
        }

        let content;
        parenthesized!(content in input);

        let method_ident = if content.peek(Token![->]) {
            content.parse::<Token![->]>()?;
            infer_method_from_trait_path(&path)?
        } else {
            let method = content.parse::<Ident>()?;
            content.parse::<Token![->]>()?;
            method
        };
        let return_ty = content.parse::<Type>()?;

        if !content.is_empty() {
            return Err(content.error("unexpected tokens in trait signature"));
        }

        Ok(OpenpitAttrItem::Capability(Box::new(CapabilitySpec {
            trait_path: path,
            method_ident,
            return_ty,
        })))
    }
}

#[derive(Clone)]
struct CapabilitySpec {
    trait_path: Path,
    method_ident: Ident,
    return_ty: Type,
}

impl quote::ToTokens for CapabilitySpec {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let trait_path = &self.trait_path;
        trait_path.to_tokens(tokens);
    }
}

fn infer_method_from_trait_path(path: &Path) -> syn::Result<Ident> {
    let Some(segment) = path.segments.last() else {
        return Err(syn::Error::new_spanned(
            path,
            "trait path must have at least one segment",
        ));
    };

    let trait_name = segment.ident.to_string();
    let Some(stripped) = trait_name.strip_prefix("Has") else {
        return Err(syn::Error::new_spanned(
            &segment.ident,
            "method inference requires a `Has*` trait name",
        ));
    };
    if stripped.is_empty() {
        return Err(syn::Error::new_spanned(
            &segment.ident,
            "trait name `Has` does not contain a method stem",
        ));
    }

    let mut snake = String::new();
    for (idx, ch) in stripped.chars().enumerate() {
        if ch.is_uppercase() {
            if idx > 0 {
                snake.push('_');
            }
            for lower in ch.to_lowercase() {
                snake.push(lower);
            }
        } else {
            snake.push(ch);
        }
    }

    Ok(Ident::new(&snake, segment.ident.span()))
}

fn impl_direct_trait(
    name: &Ident,
    generics: &Generics,
    field_ident: &Ident,
    capability: &CapabilitySpec,
) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let trait_path = &capability.trait_path;
    let method_ident = &capability.method_ident;
    let return_ty = &capability.return_ty;

    quote! {
        impl #impl_generics #trait_path for #name #ty_generics #where_clause {
            fn #method_ident(&self) -> #return_ty {
                self.#field_ident.#method_ident()
            }
        }
    }
}

fn impl_passthrough_trait(
    name: &Ident,
    generics: &Generics,
    inner_field_ident: &Ident,
    inner_ty: &Type,
    capability: &CapabilitySpec,
) -> proc_macro2::TokenStream {
    let trait_path = &capability.trait_path;
    let method_ident = &capability.method_ident;
    let return_ty = &capability.return_ty;

    let mut impl_generics = generics.clone();
    impl_generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(#inner_ty: #trait_path));
    let (impl_generics, ty_generics, where_clause) = impl_generics.split_for_impl();

    quote! {
        impl #impl_generics #trait_path for #name #ty_generics #where_clause {
            fn #method_ident(&self) -> #return_ty {
                <#inner_ty as #trait_path>::#method_ident(&self.#inner_field_ident)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use quote::quote;
    use syn::punctuated::Punctuated;
    use syn::{parse_quote, parse_str, Data, DeriveInput, Field, Fields, Path};

    use super::{
        derive_request_fields_impl, infer_method_from_trait_path, parse_openpit_items,
        register_trait_once, CapabilitySpec, OpenpitAttrItem,
    };

    fn clear_first_named_field_ident(input: &mut DeriveInput) -> bool {
        match &mut input.data {
            Data::Struct(data) => match &mut data.fields {
                Fields::Named(fields) => {
                    fields.named[0].ident = None;
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    #[test]
    fn infer_method_from_has_trait_converts_to_snake_case() {
        let path: Path = parse_quote!(crate::HasOrderPrice);
        let method = infer_method_from_trait_path(&path).expect("inference must succeed");
        assert_eq!(method.to_string(), "order_price");
    }

    #[test]
    fn infer_method_from_trait_rejects_non_has_prefix() {
        let path: Path = parse_quote!(crate::TraitWithoutPrefix);
        let err = infer_method_from_trait_path(&path).expect_err("must reject trait without Has");
        assert_eq!(
            err.to_string(),
            "method inference requires a `Has*` trait name"
        );
    }

    #[test]
    fn infer_method_from_has_rejects_empty_stem() {
        let path: Path = parse_quote!(Has);
        let err = infer_method_from_trait_path(&path).expect_err("empty method stem must reject");
        assert_eq!(
            err.to_string(),
            "trait name `Has` does not contain a method stem"
        );
    }

    #[test]
    fn infer_method_from_empty_path_rejects() {
        let path = Path {
            leading_colon: None,
            segments: Punctuated::new(),
        };
        let err = infer_method_from_trait_path(&path).expect_err("empty path must reject");
        assert_eq!(err.to_string(), "trait path must have at least one segment");
    }

    #[test]
    fn parse_openpit_items_rejects_empty_attribute() {
        let field: Field = parse_quote!(
            #[openpit()]
            operation: Operation
        );
        let err = parse_openpit_items(&field)
            .err()
            .expect("empty attribute must reject");
        assert_eq!(err.to_string(), "empty #[openpit(...)] is not allowed");
    }

    #[test]
    fn parse_openpit_items_rejects_duplicate_inner_marker() {
        let field: Field = parse_quote!(
            #[openpit(inner, inner)]
            operation: Operation
        );
        let err = parse_openpit_items(&field)
            .err()
            .expect("duplicate inner must reject");
        assert_eq!(
            err.to_string(),
            "duplicate `inner` marker in #[openpit(...)]"
        );
    }

    #[test]
    fn parse_openpit_items_parses_inner_and_capabilities() {
        let field: Field = parse_quote!(
            #[openpit(inner, crate::HasPnl(-> Result<Pnl, RequestFieldAccessError>))]
            operation: Operation
        );
        let parsed = parse_openpit_items(&field).expect("must parse valid attribute");
        assert!(parsed.inner);
        assert_eq!(parsed.capabilities.len(), 1);
        let capability = &parsed.capabilities[0];
        let trait_path = &capability.trait_path;
        assert_eq!(quote!(#trait_path).to_string(), "crate :: HasPnl");
        assert_eq!(capability.method_ident.to_string(), "pnl");
    }

    #[test]
    fn parse_openpit_items_ignores_non_openpit_attributes() {
        let field: Field = parse_quote!(
            #[serde(default)]
            operation: Operation
        );
        let parsed = parse_openpit_items(&field).expect("must ignore non-openpit attributes");
        assert!(!parsed.inner);
        assert!(parsed.capabilities.is_empty());
    }

    #[test]
    fn register_trait_once_rejects_duplicates() {
        let mut seen = std::collections::BTreeSet::new();
        let capability = CapabilitySpec {
            trait_path: parse_quote!(crate::HasInstrument),
            method_ident: parse_quote!(instrument),
            return_ty: parse_quote!(Result<&Instrument, RequestFieldAccessError>),
        };
        register_trait_once(&mut seen, &capability, &capability)
            .expect("first mapping must register");
        let err = register_trait_once(&mut seen, &capability, &capability)
            .expect_err("duplicate mapping must reject");
        assert_eq!(
            err.to_string(),
            "duplicate trait mapping for crate :: HasInstrument"
        );
    }

    #[test]
    fn derive_skips_field_without_ident_when_ast_is_malformed() {
        let mut input: DeriveInput = parse_quote!(
            struct Wrapper {
                operation: Operation,
            }
        );
        assert!(clear_first_named_field_ident(&mut input));

        let generated =
            derive_request_fields_impl(input).expect("malformed field without ident is skipped");
        assert!(generated.is_empty());
    }

    #[test]
    fn clear_first_named_field_ident_returns_false_for_non_struct() {
        let mut input: DeriveInput = parse_quote!(
            enum Wrapper {
                A,
            }
        );
        assert!(!clear_first_named_field_ident(&mut input));
    }

    #[test]
    fn clear_first_named_field_ident_returns_false_for_unnamed_struct() {
        let mut input: DeriveInput = parse_quote!(
            struct Wrapper(u64);
        );
        assert!(!clear_first_named_field_ident(&mut input));
    }

    #[test]
    fn parse_openpit_attr_item_parses_inferred_method_signature() {
        let item: OpenpitAttrItem = parse_str("HasPnl(-> Result<Pnl, RequestFieldAccessError>)")
            .expect("must parse inferred signature");
        assert_eq!(capability_method_name(item).as_deref(), Some("pnl"));
    }

    #[test]
    fn parse_openpit_attr_item_parses_explicit_method_signature() {
        let item: OpenpitAttrItem =
            parse_str("HasInstrument(instrument -> Result<&Instrument, RequestFieldAccessError>)")
                .expect("must parse explicit signature");
        assert_eq!(capability_method_name(item).as_deref(), Some("instrument"));
    }

    #[test]
    fn parse_openpit_attr_item_parses_inner_marker() {
        let item: OpenpitAttrItem = parse_str("inner").expect("must parse inner marker");
        assert_eq!(capability_method_name(item), None);
    }

    #[test]
    fn derive_request_fields_impl_generates_passthrough_for_inner_capability() {
        let input: DeriveInput = parse_quote!(
            struct Wrapper<T> {
                #[openpit(inner, HasPnl(-> Result<Pnl, RequestFieldAccessError>))]
                inner: T,
            }
        );

        let generated = derive_request_fields_impl(input).expect("derive generation must succeed");
        let generated_src = generated.to_string();
        assert!(generated_src.contains("impl < T > HasPnl for Wrapper < T > where T : HasPnl"));
        assert!(generated_src.contains("< T as HasPnl > :: pnl"));
        assert!(generated_src.contains("& self . inner"));
    }

    fn capability_method_name(item: OpenpitAttrItem) -> Option<String> {
        match item {
            OpenpitAttrItem::Capability(spec) => Some(spec.method_ident.to_string()),
            OpenpitAttrItem::Inner(_) => None,
        }
    }
}
