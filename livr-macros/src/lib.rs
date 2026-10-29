//! The `schema!` macro of the [`livr`](https://docs.rs/livr) crate: a LIVR
//! schema checked at compile time, with the type of its output inferred from
//! the rules. Use it through `livr::schema!`.
#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::collections::{HashMap, HashSet};

use livr_core::{Alias, Outcome, Registry, Validator, rule_builder, rule_fn};
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use serde_json::{Map, Value};
use syn::{Ident, Visibility};

mod aot;
mod infer;
mod names;
mod parse;

use infer::{FieldDef, Infer, State, Ty, TypeDef};
use names::{field_ident, pascal_case, variant_name};
use parse::{Body, FieldInput, SchemaInput};

/// The macro comes from the `livr-macros` crate, released with `livr` at the
/// same version; use it as `livr::schema!`.
#[proc_macro]
pub fn schema(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as SchemaInput);
    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand(input: SchemaInput) -> syn::Result<TokenStream2> {
    let SchemaInput {
        attrs,
        rules: custom,
        aliases,
        vis,
        name,
        body,
    } = input;

    let (fields, include) = match body {
        Body::Fields(fields) => (fields, None),
        Body::File(path) => {
            let (fields, include) = read_schema_file(&path)?;
            (fields, Some(include))
        }
    };

    check_rule_names(&custom, &aliases)?;
    let type_names = check_type_names(&name, &fields)?;

    // The schema is checked by the engine itself, field by field for the
    // errors to point at their field
    let mut registry = Registry::default();
    for rule in &custom {
        registry.register_rule(
            &rule.name,
            rule_builder(|_args, _ctx| Ok(rule_fn(|_, _| Outcome::Valid))),
        );
    }
    let mut alias_rules = HashMap::new();
    for alias in &aliases {
        let mut livr_alias = Alias::new(alias.name.clone(), alias.rules.clone());
        if let Some(error) = &alias.error {
            livr_alias = livr_alias.with_error(error.clone());
        }
        registry
            .register_alias(livr_alias)
            .map_err(|error| syn::Error::new(alias.span, error.message()))?;
        alias_rules.insert(alias.name.clone(), alias.rules.clone());
    }
    for alias in &aliases {
        let schema = Value::Object(Map::from_iter([("value".to_owned(), alias.rules.clone())]));
        Validator::builder(&schema)
            .registry(&registry)
            .build()
            .map_err(|error| {
                syn::Error::new(
                    alias.span,
                    format!("alias [{}]: {}", alias.name, error.message()),
                )
            })?;
    }
    for field in &fields {
        let schema = Value::Object(Map::from_iter([(field.key.clone(), field.rules.clone())]));
        Validator::builder(&schema)
            .registry(&registry)
            .build()
            .map_err(|error| syn::Error::new(field.span, error.message()))?;
    }

    let mut infer = Infer::new(&custom, &alias_rules);
    let main_name = infer.unique_name(&name.to_string());
    // Names given with `as Name` are taken before generated names get theirs
    for type_name in type_names {
        infer.reserve(type_name);
    }
    let mut field_defs = Vec::new();
    for field in &fields {
        if field.rules.as_array().is_some_and(Vec::is_empty) {
            continue;
        }
        let hint = match &field.type_name {
            Some(type_name) => {
                infer.set_next_name(type_name.to_string());
                type_name.to_string()
            }
            None => format!("{name}{}", pascal_case(&field.key)),
        };
        let state = infer.chain(
            &field.rules,
            State {
                ty: Ty::Value,
                present: false,
            },
            &hint,
        );
        infer.set_next_name(String::new());
        field_defs.push(FieldDef {
            key: field.key.clone(),
            rules: field.rules.clone(),
            state,
        });
    }
    let schema_fields: Vec<TokenStream2> = field_defs
        .iter()
        .map(|field| {
            let key = &field.key;
            let is_list = matches!(field.state.ty, Ty::List(..));
            quote! { ::livr::SchemaField::new(#key, #is_list) }
        })
        .collect();
    infer.defs.push(TypeDef::Struct {
        name: main_name.clone(),
        fields: field_defs,
    });

    let schema: Map<String, Value> = fields
        .iter()
        .map(|field| (field.key.clone(), field.rules.clone()))
        .collect();
    let schema_json = serde_json::to_string(&schema).expect("JSON");

    let codegen = Gen {
        attrs: &attrs,
        vis: &vis,
        infer: &infer,
        main: &main_name,
    };
    let defs = infer.defs.iter().map(|def| codegen.def(def));

    let registrations = custom.iter().map(|rule| {
        let rule_name = &rule.name;
        let builder = &rule.builder;
        quote! { registry.register_rule(#rule_name, #builder); }
    });
    let alias_registrations = aliases.iter().map(|alias| {
        let alias_name = &alias.name;
        let rules = serde_json::to_string(&alias.rules).expect("JSON");
        let with_error = alias
            .error
            .as_ref()
            .map(|error| quote! { .with_error(#error) });
        quote! {
            registry
                .register_alias(
                    ::livr::Alias::new(#alias_name, ::livr::__private::json(#rules)) #with_error
                )
                .expect("alias checked by schema!");
        }
    });

    Ok(quote! {
        #(#defs)*

        #include

        impl #main_name {
            /// The LIVR schema, as JSON.
            pub const SCHEMA: &'static str = #schema_json;

            /// The fields of the output, in the order of the schema: their
            /// names in the data and whether they are lists.
            pub const FIELDS: &'static [::livr::SchemaField] = &[#(#schema_fields),*];

            /// The validator of the schema, built on first use, for an
            /// output that is not typed.
            pub fn validator() -> &'static ::livr::Validator {
                static VALIDATOR: ::livr::__private::LazyLock<::livr::Validator> =
                    ::livr::__private::LazyLock::new(|| {
                        ::livr::Validator::builder(&::livr::__private::json(#main_name::SCHEMA))
                            .registry(#main_name::__livr_registry())
                            .build()
                            .expect("schema checked by schema!")
                    });
                &VALIDATOR
            }

            #[doc(hidden)]
            pub fn __livr_registry() -> &'static ::livr::Registry {
                static REGISTRY: ::livr::__private::LazyLock<::livr::Registry> =
                    ::livr::__private::LazyLock::new(|| {
                        #[allow(unused_mut)]
                        let mut registry = <::livr::Registry as ::core::default::Default>::default();
                        #(#registrations)*
                        #(#alias_registrations)*
                        registry
                    });
                &REGISTRY
            }

            /// Validates the data.
            ///
            /// # Errors
            ///
            /// The errors of the invalid fields, or `FORMAT_ERROR` when the
            /// data is not an object.
            pub fn validate(data: &::livr::Value) -> ::core::result::Result<Self, ::livr::Errors> {
                match data {
                    ::livr::Value::Object(fields) => {
                        <Self as ::livr::TypedObject>::validate_fields(fields).map_err(::livr::Errors::Fields)
                    }
                    _ => ::core::result::Result::Err(::livr::Errors::Format),
                }
            }

            /// Validates owned data, like data just parsed: faster than
            /// `validate`, values are moved instead of copied.
            ///
            /// # Errors
            ///
            /// The errors of the invalid fields, or `FORMAT_ERROR` when the
            /// data is not an object.
            pub fn validate_owned(data: ::livr::Value) -> ::core::result::Result<Self, ::livr::Errors> {
                match data {
                    ::livr::Value::Object(fields) => {
                        <Self as ::livr::TypedObject>::validate_fields_owned(fields).map_err(::livr::Errors::Fields)
                    }
                    _ => ::core::result::Result::Err(::livr::Errors::Format),
                }
            }
        }

        impl ::livr::Schema for #main_name {
            const SCHEMA: &'static str = #schema_json;

            const FIELDS: &'static [::livr::SchemaField] = #main_name::FIELDS;

            fn validator() -> &'static ::livr::Validator {
                #main_name::validator()
            }

            fn validate(data: &::livr::Value) -> ::core::result::Result<Self, ::livr::Errors> {
                #main_name::validate(data)
            }

            fn validate_owned(data: ::livr::Value) -> ::core::result::Result<Self, ::livr::Errors> {
                #main_name::validate_owned(data)
            }
        }
    })
}

// A name used twice by custom rules and aliases, also as camelCase forms of
// each other (`foo_bar` and `fooBar`), would mean one rule for the types and
// another at run time: the registry keeps the last one registered, and every
// rule is registered under its camelCase name too
fn check_rule_names(custom: &[parse::CustomRule], aliases: &[parse::AliasAttr]) -> syn::Result<()> {
    let names: Vec<(&str, Span, &str)> = custom
        .iter()
        .map(|rule| (rule.name.as_str(), rule.span, "a custom rule"))
        .chain(
            aliases
                .iter()
                .map(|alias| (alias.name.as_str(), alias.span, "an alias")),
        )
        .collect();
    for (i, &(name, span, _)) in names.iter().enumerate() {
        let camelized = livr_core::util::camelize(name);
        let earlier = names[..i]
            .iter()
            .find(|(other, _, _)| *other == name || livr_core::util::camelize(other) == camelized);
        if let Some(&(other, _, kind)) = earlier {
            let message = if other == name {
                format!("`{name}` is already the name of {kind}")
            } else {
                format!(
                    "`{name}` is the same rule name as `{other}` in camelCase, already the name of {kind}"
                )
            };
            return Err(syn::Error::new(span, message));
        }
    }
    Ok(())
}

// The names of `as Name`: unique, and not the name of the schema. Generated
// names avoid them, so they are used as they are
fn check_type_names(main: &Ident, fields: &[FieldInput]) -> syn::Result<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for type_name in fields.iter().filter_map(|field| field.type_name.as_ref()) {
        let name = type_name.to_string();
        if *main == name {
            return Err(syn::Error::new(
                type_name.span(),
                format!("`{name}` is already the name of the schema"),
            ));
        }
        if names.contains(&name) {
            return Err(syn::Error::new(
                type_name.span(),
                format!("the type name `{name}` is used twice"),
            ));
        }
        names.push(name);
    }
    Ok(names)
}

fn read_schema_file(path: &syn::LitStr) -> syn::Result<(Vec<FieldInput>, TokenStream2)> {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let full_path = std::path::Path::new(&dir).join(path.value());
    let text = std::fs::read_to_string(&full_path).map_err(|error| {
        syn::Error::new(
            path.span(),
            format!("cannot read {}: {error}", full_path.display()),
        )
    })?;
    let schema: Value = serde_json::from_str(&text).map_err(|error| {
        syn::Error::new(
            path.span(),
            format!("invalid JSON in {}: {error}", full_path.display()),
        )
    })?;
    let Value::Object(schema) = schema else {
        return Err(syn::Error::new(
            path.span(),
            "a schema must be a JSON object",
        ));
    };
    let fields = schema
        .into_iter()
        .map(|(key, rules)| FieldInput {
            key,
            span: path.span(),
            rules,
            type_name: None,
        })
        .collect();
    // Rebuilds when the file changes
    let full_path = full_path.to_string_lossy().into_owned();
    Ok((
        fields,
        quote! { const _: &str = ::core::include_str!(#full_path); },
    ))
}

struct Gen<'a> {
    attrs: &'a [syn::Attribute],
    vis: &'a Visibility,
    infer: &'a Infer<'a>,
    // The main struct, which has the registry
    main: &'a Ident,
}

// How a field is validated by typed code (at compile time: size does not matter)
#[allow(clippy::large_enum_variant)]
enum FieldKind {
    // By its rules, then converted
    Rules(Value),
    // By its rules up to the last one, then validated by a typed object
    Nested(Value, Ident),
    ListOfObjects(Value, Ident),
    ListOf(Value, Value, State),
}

impl Gen<'_> {
    fn ty(&self, ty: &Ty) -> TokenStream2 {
        match ty {
            Ty::Value => quote! { ::livr::Value },
            Ty::String => quote! { ::std::string::String },
            Ty::I64 => quote! { i64 },
            Ty::F64 => quote! { f64 },
            Ty::Bool => quote! { bool },
            Ty::Map => quote! { ::livr::Map<::std::string::String, ::livr::Value> },
            Ty::List(item, present) => {
                let item = self.state_ty(&State {
                    ty: (**item).clone(),
                    present: *present,
                });
                quote! { ::std::vec::Vec<#item> }
            }
            Ty::Named(name) => quote! { #name },
            Ty::Custom(ty) => quote! { #ty },
        }
    }

    fn state_ty(&self, state: &State) -> TokenStream2 {
        let ty = self.ty(&state.ty);
        if state.present {
            ty
        } else {
            quote! { ::core::option::Option<#ty> }
        }
    }

    fn def(&self, def: &TypeDef) -> TokenStream2 {
        let attrs = self.attrs;
        let vis = self.vis;
        match def {
            TypeDef::Struct { name, fields } => self.struct_def(name, fields),
            TypeDef::StrEnum { name, values } => {
                let variants = unique_variants(values.iter().map(|value| variant_name(value)));
                quote! {
                    #(#attrs)*
                    #[derive(::livr::__private::serde::Serialize)]
                    #[serde(crate = "::livr::__private::serde")]
                    #vis enum #name {
                        #(#[serde(rename = #values)] #variants,)*
                    }

                    impl #name {
                        /// The value as a string.
                        #vis const fn as_str(&self) -> &'static str {
                            match self { #(Self::#variants => #values,)* }
                        }
                    }

                    impl ::core::fmt::Display for #name {
                        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                            f.write_str(self.as_str())
                        }
                    }

                    impl ::livr::FromLivr for #name {
                        fn from_livr(value: ::livr::Value) -> ::core::result::Result<Self, ::livr::ConvertError> {
                            <Self as ::livr::FromLivrRef>::from_livr_ref(&value)
                        }
                    }

                    impl ::livr::FromLivrRef for #name {
                        fn from_livr_ref(value: &::livr::Value) -> ::core::result::Result<Self, ::livr::ConvertError> {
                            match value.as_str() {
                                #(::core::option::Option::Some(#values) => ::core::result::Result::Ok(Self::#variants),)*
                                _ => ::core::result::Result::Err(::livr::ConvertError::new("an allowed value")),
                            }
                        }
                    }
                }
            }
            TypeDef::Tagged {
                name,
                selector,
                variants,
            } => {
                let values: Vec<&String> = variants.iter().map(|(value, _)| value).collect();
                let structs: Vec<&Ident> = variants.iter().map(|(_, ident)| ident).collect();
                let idents = unique_variants(values.iter().map(|value| variant_name(value)));
                quote! {
                    #(#attrs)*
                    #[derive(::livr::__private::serde::Serialize)]
                    #[serde(crate = "::livr::__private::serde", tag = #selector)]
                    #vis enum #name {
                        #(#[serde(rename = #values)] #idents(#structs),)*
                    }

                    impl #name {
                        fn __livr_selector(value: &::livr::Value) -> ::core::option::Option<::std::string::String> {
                            match value {
                                ::livr::Value::Object(fields) => {
                                    fields.get(#selector).map(|value| ::livr::util::to_js_string(value).into_owned())
                                }
                                _ => ::core::option::Option::None,
                            }
                        }
                    }

                    impl ::livr::FromLivr for #name {
                        fn from_livr(value: ::livr::Value) -> ::core::result::Result<Self, ::livr::ConvertError> {
                            match Self::__livr_selector(&value).as_deref() {
                                #(::core::option::Option::Some(#values) => {
                                    <#structs as ::livr::FromLivr>::from_livr(value).map(Self::#idents)
                                })*
                                _ => ::core::result::Result::Err(::livr::ConvertError::new("an object with a known selector")),
                            }
                        }
                    }

                    impl ::livr::FromLivrRef for #name {
                        fn from_livr_ref(value: &::livr::Value) -> ::core::result::Result<Self, ::livr::ConvertError> {
                            match Self::__livr_selector(value).as_deref() {
                                #(::core::option::Option::Some(#values) => {
                                    <#structs as ::livr::FromLivrRef>::from_livr_ref(value).map(Self::#idents)
                                })*
                                _ => ::core::result::Result::Err(::livr::ConvertError::new("an object with a known selector")),
                            }
                        }
                    }
                }
            }
            TypeDef::Union { name, variants } => {
                let tys: Vec<TokenStream2> = variants.iter().map(|ty| self.ty(ty)).collect();
                let idents = unique_variants(variants.iter().map(|ty| self.union_variant_name(ty)));
                let (last_ty, first_tys) = tys.split_last().expect("variants");
                let (last_ident, first_idents) = idents.split_last().expect("variants");
                quote! {
                    #(#attrs)*
                    #[derive(::livr::__private::serde::Serialize)]
                    #[serde(crate = "::livr::__private::serde", untagged)]
                    #vis enum #name {
                        #(#idents(#tys),)*
                    }

                    // The first type that fits: the one of the first valid rule set
                    impl ::livr::FromLivr for #name {
                        fn from_livr(value: ::livr::Value) -> ::core::result::Result<Self, ::livr::ConvertError> {
                            #(if let ::core::result::Result::Ok(value) = <#first_tys as ::livr::FromLivrRef>::from_livr_ref(&value) {
                                return ::core::result::Result::Ok(Self::#first_idents(value));
                            })*
                            <#last_ty as ::livr::FromLivr>::from_livr(value).map(Self::#last_ident)
                        }
                    }

                    impl ::livr::FromLivrRef for #name {
                        fn from_livr_ref(value: &::livr::Value) -> ::core::result::Result<Self, ::livr::ConvertError> {
                            #(if let ::core::result::Result::Ok(value) = <#first_tys as ::livr::FromLivrRef>::from_livr_ref(value) {
                                return ::core::result::Result::Ok(Self::#first_idents(value));
                            })*
                            <#last_ty as ::livr::FromLivrRef>::from_livr_ref(value).map(Self::#last_ident)
                        }
                    }
                }
            }
        }
    }

    fn field_kind(&self, field: &FieldDef) -> FieldKind {
        let rules = match &field.rules {
            Value::Array(rules) => rules.clone(),
            rule => vec![rule.clone()],
        };
        if let Some((last, prefix)) = rules.split_last()
            && let Some((name, args)) = infer::rule_parts(last)
        {
            let prefix = Value::Array(prefix.to_vec());
            match (self.infer.builtin_name(name), &field.state.ty) {
                (Some("nested_object"), ty @ Ty::Named(ident)) if self.infer.is_struct(ty) => {
                    return FieldKind::Nested(prefix, ident.clone());
                }
                (Some("list_of_objects"), Ty::List(item, _)) if self.infer.is_struct(item) => {
                    if let Ty::Named(ident) = &**item {
                        return FieldKind::ListOfObjects(prefix, ident.clone());
                    }
                }
                (Some("list_of"), Ty::List(item, present)) => {
                    let item_rules = match args {
                        [rules @ Value::Array(_)] => rules.clone(),
                        rules => Value::Array(rules.to_vec()),
                    };
                    let item = State {
                        ty: (**item).clone(),
                        present: *present,
                    };
                    return FieldKind::ListOf(prefix, item_rules, item);
                }
                _ => {}
            }
        }
        FieldKind::Rules(field.rules.clone())
    }

    fn struct_def(&self, name: &Ident, fields: &[FieldDef]) -> TokenStream2 {
        let attrs = self.attrs;
        let vis = self.vis;
        let mut idents = HashSet::new();
        let mut decls = Vec::new();
        let mut field_idents = Vec::new();
        let mut keys = Vec::new();
        let mut from_value = Vec::new();
        let mut from_ref = Vec::new();
        let mut checked = Vec::new();
        // A function for the rules of each field and of each list item
        let mut rules_fns = Vec::new();
        let mut rules_fn = |rules: &Value| {
            let ident = format_ident!("__livr_rules_{}", rules_fns.len());
            rules_fns.push(self.rules_fn(&ident, rules));
            ident
        };
        let mut reads_ctx = false;
        for field in fields {
            let (mut ident, mut renamed) = field_ident(&field.key);
            let mut i = 2;
            while !idents.insert(ident.to_string()) {
                ident = format_ident!("{}_{i}", field_ident(&field.key).0);
                renamed = true;
                i += 1;
            }
            let key = &field.key;
            let rename = renamed.then(|| quote! { #[serde(rename = #key)] });
            let skip = (!field.state.present).then(
                || quote! { #[serde(skip_serializing_if = "::core::option::Option::is_none")] },
            );
            let ty = self.state_ty(&field.state);
            decls.push(quote! { #rename #skip #vis #ident: #ty });
            from_value.push(quote! {
                ::livr::FromLivr::from_livr(fields.remove(#key).unwrap_or(::livr::Value::Null))
                    .map_err(|error| error.in_field(#key))?
            });
            from_ref.push(quote! {
                ::livr::FromLivrRef::from_livr_ref(fields.get(#key).unwrap_or(&NULL))
                    .map_err(|error| error.in_field(#key))?
            });
            let required = field
                .state
                .present
                .then(|| quote! { .and_then(::livr::__private::required) });
            let kind = self.field_kind(field);
            let (FieldKind::Rules(rules)
            | FieldKind::Nested(rules, _)
            | FieldKind::ListOfObjects(rules, _)
            | FieldKind::ListOf(rules, ..)) = &kind;
            reads_ctx |= self.reads_ctx(rules);
            let run = rules_fn(rules);
            // The data is read once, into a slot per field (keys are unique)
            let slot = proc_macro2::Literal::usize_unsuffixed(keys.len());
            let checked_value = quote! { Self::#run(get(#slot), ctx) };
            let check = match kind {
                FieldKind::Rules(_) => {
                    quote! { ::livr::__private::field::<#ty>(#checked_value) }
                }
                FieldKind::Nested(_, nested) => {
                    quote! { ::livr::__private::nested::<#nested>(#checked_value) #required }
                }
                FieldKind::ListOfObjects(_, item) => {
                    quote! { ::livr::__private::list_of_objects::<#item>(#checked_value) #required }
                }
                FieldKind::ListOf(_, item_rules, item) => {
                    let item_reads_ctx = self.reads_ctx(&item_rules);
                    let item_run = rules_fn(&item_rules);
                    let item_ty = self.state_ty(&item);
                    quote! {
                        ::livr::__private::list_of::<#item_ty>(#checked_value, #item_reads_ctx, Self::#item_run)
                            #required
                    }
                }
            };
            checked.push(check);
            field_idents.push(ident);
            keys.push(key.clone());
        }
        let slot_count = proc_macro2::Literal::usize_unsuffixed(keys.len());
        let slot_indexes: Vec<proc_macro2::Literal> = (0..keys.len())
            .map(proc_macro2::Literal::usize_unsuffixed)
            .collect();
        // Beyond this, a lookup per field is cheaper than a pass over the data
        let max_scan = proc_macro2::Literal::usize_unsuffixed(2 * keys.len() + 4);
        let locals: Vec<Ident> = (0..field_idents.len())
            .map(|i| format_ident!("__field_{i}"))
            .collect();
        quote! {
            #(#attrs)*
            #[derive(::livr::__private::serde::Serialize)]
            #[serde(crate = "::livr::__private::serde")]
            #vis struct #name {
                #(#decls,)*
            }

            impl #name {
                #(#rules_fns)*
            }

            impl #name {
                // The fields from `get`: borrowed from borrowed data, taken
                // from owned data
                #[allow(unused_variables, unused_mut)]
                fn __livr_fields<'v>(
                    mut get: impl ::core::ops::FnMut(usize) -> ::std::borrow::Cow<'v, ::livr::Value>,
                    ctx: &::livr::Ctx<'_>,
                ) -> ::core::result::Result<Self, ::std::collections::BTreeMap<::std::string::String, ::livr::ErrorTree>> {
                    let mut errors = <::livr::__private::Fields as ::core::default::Default>::default();
                    #(let #locals = errors.take(#keys, #checked);)*
                    errors.finish()?;
                    match (#(#locals,)*) {
                        (#(::core::option::Option::Some(#locals),)*) => {
                            ::core::result::Result::Ok(Self { #(#field_idents: #locals,)* })
                        }
                        #[allow(unreachable_patterns)]
                        _ => ::core::result::Result::Err(::std::collections::BTreeMap::new()),
                    }
                }
            }

            #[allow(unused_variables, unused_mut)]
            impl ::livr::TypedObject for #name {
                fn validate_fields(
                    data: &::livr::Map<::std::string::String, ::livr::Value>,
                ) -> ::core::result::Result<Self, ::std::collections::BTreeMap<::std::string::String, ::livr::ErrorTree>> {
                    let mut slots: [::core::option::Option<&::livr::Value>; #slot_count] = [::core::option::Option::None; #slot_count];
                    // One pass over small objects: a `match` on the key is
                    // cheaper than a lookup per field
                    if data.len() <= #max_scan {
                        for (key, value) in data {
                            match key.as_str() {
                                #(#keys => slots[#slot_indexes] = ::core::option::Option::Some(value),)*
                                _ => {}
                            }
                        }
                    } else {
                        #(slots[#slot_indexes] = data.get(#keys);)*
                    }
                    Self::__livr_fields(|i| ::livr::__private::slot(slots[i]), &::livr::Ctx::object(data))
                }

                fn validate_fields_owned(
                    mut data: ::livr::Map<::std::string::String, ::livr::Value>,
                ) -> ::core::result::Result<Self, ::std::collections::BTreeMap<::std::string::String, ::livr::ErrorTree>> {
                    // Rules that read other fields need them all
                    if #reads_ctx {
                        return <Self as ::livr::TypedObject>::validate_fields(&data);
                    }
                    let mut slots: [::core::option::Option<::livr::Value>; #slot_count] = [const { ::core::option::Option::None }; #slot_count];
                    for (key, value) in data {
                        match key.as_str() {
                            #(#keys => slots[#slot_indexes] = ::core::option::Option::Some(value),)*
                            _ => {}
                        }
                    }
                    Self::__livr_fields(|i| ::livr::__private::take_slot(&mut slots[i]), &::livr::Ctx::none())
                }
            }

            impl ::livr::FromLivr for #name {
                fn from_livr(value: ::livr::Value) -> ::core::result::Result<Self, ::livr::ConvertError> {
                    #[allow(unused_mut, unused_variables)]
                    let ::livr::Value::Object(mut fields) = value else {
                        return ::core::result::Result::Err(::livr::ConvertError::new("an object"));
                    };
                    ::core::result::Result::Ok(Self { #(#field_idents: #from_value,)* })
                }
            }

            impl ::livr::FromLivrRef for #name {
                fn from_livr_ref(value: &::livr::Value) -> ::core::result::Result<Self, ::livr::ConvertError> {
                    const NULL: ::livr::Value = ::livr::Value::Null;
                    #[allow(unused_variables)]
                    let ::livr::Value::Object(fields) = value else {
                        return ::core::result::Result::Err(::livr::ConvertError::new("an object"));
                    };
                    ::core::result::Result::Ok(Self { #(#field_idents: #from_ref,)* })
                }
            }
        }
    }

    fn union_variant_name(&self, ty: &Ty) -> String {
        match ty {
            Ty::Value => "Value".to_owned(),
            Ty::String => "String".to_owned(),
            Ty::I64 => "Integer".to_owned(),
            Ty::F64 => "Number".to_owned(),
            Ty::Bool => "Bool".to_owned(),
            Ty::Map => "Object".to_owned(),
            Ty::List(..) => "List".to_owned(),
            Ty::Named(name) => self.infer.short_name(name),
            Ty::Custom(_) => "Custom".to_owned(),
        }
    }
}

fn unique_variants(names: impl Iterator<Item = String>) -> Vec<Ident> {
    let mut seen = HashSet::new();
    names
        .map(|name| {
            let mut unique = name.clone();
            let mut i = 2;
            while !seen.insert(unique.clone()) {
                unique = format!("{name}{i}");
                i += 1;
            }
            Ident::new(&unique, Span::call_site())
        })
        .collect()
}
