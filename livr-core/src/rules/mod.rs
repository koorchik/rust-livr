//! The built-in rules of the LIVR specification, one per file. Their checks
//! are public, so typed code can call them directly.

pub mod checksum;
pub mod common;
pub mod format;
pub mod list;
pub mod meta;
pub mod modifiers;
pub mod numeric;
pub mod special;
pub mod string;

/// What rule files use: the rule contract and the helpers.
pub(crate) mod prelude {
    pub(crate) use crate::compile::{
        Chain, Checked, CompileCtx, ObjectValidator, outcome, run_items,
    };
    pub(crate) use crate::error::{ErrorTree, SchemaError};
    pub(crate) use crate::reference::Reference;
    pub(crate) use crate::rule::{Ctx, Outcome};
    pub(crate) use crate::text::{is_hex_digit, is_ipv4, is_ipv6};
    pub(crate) use crate::time::{
        Clock, DAY_MS, format_datetime, parse_date, parse_datetime, parse_time,
    };
    pub(crate) use crate::util::{
        Num, arg, char_len, is_no_value, is_primitive, js_trim, number_arg, parse_number,
        string_arg, to_js_string,
    };
    pub(crate) use serde_json::{Map, Value};
    pub(crate) use std::borrow::Cow;

    /// Valid for a string, the string of another primitive value otherwise:
    /// string rules output strings.
    #[inline]
    pub(crate) fn string_output(value: &Value, str: Cow<'_, str>) -> Outcome {
        if value.is_string() {
            Outcome::Valid
        } else {
            Outcome::Value(Value::String(str.into_owned()))
        }
    }

    /// A format or checksum rule (LIVR 2.5): checks the string form of a
    /// string or a number and outputs it; a boolean gets the code of the
    /// rule, an object or a list `FORMAT_ERROR`, empty values pass.
    #[inline]
    pub(crate) fn check_format(
        value: &Value,
        code: &'static str,
        is_valid: impl FnOnce(&[u8]) -> bool,
    ) -> Outcome {
        match value {
            Value::String(str) => {
                if str.is_empty() || is_valid(str.as_bytes()) {
                    Outcome::Valid
                } else {
                    Outcome::code(code)
                }
            }
            Value::Number(_) => {
                let str = to_js_string(value);
                if is_valid(str.as_bytes()) {
                    Outcome::Value(Value::String(str.into_owned()))
                } else {
                    Outcome::code(code)
                }
            }
            Value::Null => Outcome::Valid,
            Value::Bool(_) => Outcome::code(code),
            Value::Array(_) | Value::Object(_) => Outcome::code("FORMAT_ERROR"),
        }
    }

    /// Fails for a rule of LIVR 2.5 that takes no arguments and got some.
    /// `{"rule": null}` is one `null` argument, so it fails too.
    pub(crate) fn no_args(args: &[Value], rule: &str) -> Result<(), SchemaError> {
        if args.is_empty() {
            Ok(())
        } else {
            Err(SchemaError::new(format!(
                "rule [{rule}] takes no arguments, got {}",
                Value::Array(args.to_vec())
            )))
        }
    }

    /// The first argument of a rule of LIVR 2.5 that takes one; the
    /// arguments after it are ignored, unchecked.
    pub(crate) fn first_arg<'a>(args: &'a [Value], rule: &str) -> Result<&'a Value, SchemaError> {
        args.first()
            .ok_or_else(|| SchemaError::new(format!("rule [{rule}] takes an argument, got none")))
    }

    /// A non-negative integer argument: a JSON number, or a string with a
    /// number (numbers and strings are interchangeable in arguments).
    pub(crate) fn count_arg(value: &Value, rule: &str) -> Result<u64, SchemaError> {
        let count = match value {
            Value::Number(number) => number.as_u64().or_else(|| count_of(number.as_f64()?)),
            Value::String(str) => parse_number(str).and_then(|number| match number {
                Num::Int(int) => u64::try_from(int).ok(),
                number => count_of(number.as_f64()),
            }),
            _ => None,
        };
        count.ok_or_else(|| {
            SchemaError::new(format!(
                "rule [{rule}] expects a non-negative integer, got {value}"
            ))
        })
    }

    // 2.0 is 2, as in JavaScript
    fn count_of(float: f64) -> Option<u64> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        (float >= 0.0 && float.fract() == 0.0 && float < 9e15).then_some(float as u64)
    }

    /// A string argument of a rule of LIVR 2.5: a string, or the string form
    /// of a number (numbers and strings are interchangeable in arguments).
    /// Booleans, `null`, lists and objects are not strings.
    pub(crate) fn text_arg(value: &Value) -> Option<Cow<'_, str>> {
        match value {
            Value::String(str) => Some(Cow::Borrowed(str)),
            Value::Number(_) => Some(to_js_string(value)),
            _ => None,
        }
    }

    /// The options object of a rule: fails for keys not in `known`.
    pub(crate) fn options_arg<'a>(
        value: &'a Value,
        known: &[&str],
        rule: &str,
    ) -> Result<&'a Map<String, Value>, SchemaError> {
        let Value::Object(options) = value else {
            return Err(SchemaError::new(format!(
                "rule [{rule}] expects an object of options, got {value}"
            )));
        };
        if let Some(key) = options.keys().find(|key| !known.contains(&key.as_str())) {
            return Err(SchemaError::new(format!(
                "rule [{rule}] got an unknown option {key:?}"
            )));
        }
        Ok(options)
    }
}

use crate::compile::{AliasRule, Checked, checked};
use crate::registry::BuildFn;
use crate::rule::{Ctx, Outcome, Rule};
use serde_json::Value;
use std::borrow::Cow;

macro_rules! built_in_rules {
    (
        rules: [$($group:ident::$file:ident::$rule:ident => $name:literal,)*]
        metarules: [$($meta_group:ident::$meta_file:ident::$meta:ident => $meta_name:literal,)*]
        ext_rules: [$($ext_group:ident::$ext_file:ident::$ext:ident => $ext_name:literal,)*]
        ext_metarules: [$($ext_meta_group:ident::$ext_meta_file:ident::$ext_meta:ident => $ext_meta_name:literal,)*]
        ext_ctx_metarules: [$($ctx_group:ident::$ctx_file:ident::$ctx_meta:ident => $ctx_name:literal,)*]
    ) => {
        $(pub use $group::$file::$rule;)*
        $(pub use $meta_group::$meta_file::$meta;)*
        $(pub use $ext_group::$ext_file::$ext;)*
        $(pub use $ext_meta_group::$ext_meta_file::$ext_meta;)*
        $(pub use $ctx_group::$ctx_file::$ctx_meta;)*

        /// A compiled rule: built-in rules are dispatched without virtual
        /// calls.
        pub(crate) enum CompiledRule {
            $($rule($rule),)*
            $($meta($meta),)*
            // The rules of LIVR 2.5 in an enum of their own: the match of
            // `run` stays as small as for LIVR 2.0, which keeps the code of
            // the 2.0 rules as fast as it was (a match over all rules was
            // 5-15% slower)
            Ext(ExtRule),
            Alias(AliasRule),
            Custom(Box<dyn Rule>),
        }

        /// A rule of LIVR 2.5.
        pub(crate) enum ExtRule {
            $($ext($ext),)*
            $($ext_meta($ext_meta),)*
            $($ctx_meta($ctx_meta),)*
        }

        impl std::fmt::Debug for CompiledRule {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$rule(rule) => std::fmt::Debug::fmt(rule, f),)*
                    $(Self::$meta(rule) => std::fmt::Debug::fmt(rule, f),)*
                    Self::Ext(rule) => std::fmt::Debug::fmt(rule, f),
                    Self::Alias(rule) => std::fmt::Debug::fmt(rule, f),
                    // A custom rule can be any type
                    Self::Custom(_) => f.write_str("Custom(..)"),
                }
            }
        }

        impl std::fmt::Debug for ExtRule {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$ext(rule) => std::fmt::Debug::fmt(rule, f),)*
                    $(Self::$ext_meta(rule) => std::fmt::Debug::fmt(rule, f),)*
                    $(Self::$ctx_meta(rule) => std::fmt::Debug::fmt(rule, f),)*
                }
            }
        }

        impl CompiledRule {
            /// Runs the rule: metarules move the unchanged parts of an owned
            /// value to the output, other rules check the value.
            #[inline]
            pub(crate) fn run<'v>(&self, value: Cow<'v, Value>, ctx: &Ctx<'_>) -> Checked<'v> {
                match self {
                    $(Self::$rule(rule) => {
                        let outcome = rule.check(&value, ctx);
                        checked(outcome, value)
                    })*
                    $(Self::$meta(rule) => rule.run(value),)*
                    Self::Ext(rule) => rule.run(value, ctx),
                    Self::Alias(rule) => rule.run(value, ctx),
                    Self::Custom(rule) => {
                        let outcome = rule.check(&value, ctx);
                        checked(outcome, value)
                    }
                }
            }

            /// Whether the rule reads other fields of the object: aliases
            /// and `or` see the object of the field, so they read it when
            /// their rules do.
            pub(crate) fn reads_ctx(&self) -> bool {
                match self {
                    Self::EqualToField(_) | Self::Custom(_) => true,
                    Self::Ext(ExtRule::RequiredIf(_) | ExtRule::When(_)) => true,
                    Self::Alias(rule) => rule.reads_ctx(),
                    Self::Or(rule) => rule.reads_ctx(),
                    _ => false,
                }
            }
        }

        impl ExtRule {
            #[inline(never)]
            fn run<'v>(&self, value: Cow<'v, Value>, ctx: &Ctx<'_>) -> Checked<'v> {
                match self {
                    $(Self::$ext(rule) => {
                        let outcome = rule.check(&value, ctx);
                        checked(outcome, value)
                    })*
                    $(Self::$ext_meta(rule) => rule.run(value),)*
                    $(Self::$ctx_meta(rule) => rule.run(value, ctx),)*
                }
            }
        }

        $(
            impl Rule for $rule {
                #[inline]
                fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
                    $rule::check(self, value, ctx)
                }
            }
        )*
        $(
            impl Rule for $meta {
                #[inline]
                fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
                    $meta::check(self, value, ctx)
                }
            }
        )*
        $(
            impl Rule for $ext {
                #[inline]
                fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
                    $ext::check(self, value, ctx)
                }
            }
        )*
        $(
            impl Rule for $ext_meta {
                #[inline]
                fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
                    $ext_meta::check(self, value, ctx)
                }
            }
        )*
        $(
            impl Rule for $ctx_meta {
                #[inline]
                fn check(&self, value: &Value, ctx: &Ctx<'_>) -> Outcome {
                    $ctx_meta::check(self, value, ctx)
                }
            }
        )*

        /// The built-in rules by their snake_case names.
        pub(crate) const BUILT_IN_RULES: &[(&str, BuildFn)] = &[
            $(($name, |args, ctx| Ok(CompiledRule::$rule($rule::build(args, ctx)?))),)*
            $(($meta_name, |args, ctx| Ok(CompiledRule::$meta($meta::build(args, ctx)?))),)*
            $(($ext_name, |args, ctx| Ok(CompiledRule::Ext(ExtRule::$ext($ext::build(args, ctx)?)))),)*
            $(($ext_meta_name, |args, ctx| Ok(CompiledRule::Ext(ExtRule::$ext_meta($ext_meta::build(args, ctx)?)))),)*
            $(($ctx_name, |args, ctx| Ok(CompiledRule::Ext(ExtRule::$ctx_meta($ctx_meta::build(args, ctx)?)))),)*
        ];
    };
}

built_in_rules! {
    rules: [
        common::required::Required => "required",
        common::not_empty::NotEmpty => "not_empty",
        common::not_empty_list::NotEmptyList => "not_empty_list",
        common::any_object::AnyObject => "any_object",

        string::string::StringRule => "string",
        string::eq::EqRule => "eq",
        string::one_of::OneOf => "one_of",
        string::max_length::MaxLength => "max_length",
        string::min_length::MinLength => "min_length",
        string::length_between::LengthBetween => "length_between",
        string::length_equal::LengthEqual => "length_equal",
        string::like::Like => "like",

        numeric::integer::Integer => "integer",
        numeric::positive_integer::PositiveInteger => "positive_integer",
        numeric::decimal::Decimal => "decimal",
        numeric::positive_decimal::PositiveDecimal => "positive_decimal",
        numeric::max_number::MaxNumber => "max_number",
        numeric::min_number::MinNumber => "min_number",
        numeric::number_between::NumberBetween => "number_between",

        special::email::Email => "email",
        special::url::Url => "url",
        special::iso_date::IsoDate => "iso_date",
        special::equal_to_field::EqualToField => "equal_to_field",

        // `or` tries its rule sets on the borrowed value
        meta::or::Or => "or",

        modifiers::trim::Trim => "trim",
        modifiers::to_lc::ToLc => "to_lc",
        modifiers::to_uc::ToUc => "to_uc",
        modifiers::remove::Remove => "remove",
        modifiers::leave_only::LeaveOnly => "leave_only",
        modifiers::default::DefaultRule => "default",
    ]
    // Rules that run on owned values too, moving what does not change
    metarules: [
        meta::nested_object::NestedObject => "nested_object",
        meta::variable_object::VariableObject => "variable_object",
        meta::list_of::ListOf => "list_of",
        meta::list_of_objects::ListOfObjects => "list_of_objects",
        meta::list_of_different_objects::ListOfDifferentObjects => "list_of_different_objects",
    ]
    // LIVR 2.5
    ext_rules: [
        common::boolean::Boolean => "boolean",
        common::any::Any => "any",
        common::required_if::RequiredIf => "required_if",

        string::not_one_of::NotOneOf => "not_one_of",
        string::digits::Digits => "digits",
        string::starts_with::StartsWith => "starts_with",
        string::ends_with::EndsWith => "ends_with",
        string::contains::Contains => "contains",

        numeric::max_decimals::MaxDecimals => "max_decimals",

        special::iso_datetime::IsoDatetime => "iso_datetime",
        special::iso_time::IsoTime => "iso_time",

        meta::keys::Keys => "keys",

        list::list_length::ListLength => "list_length",
        list::unique::Unique => "unique",

        modifiers::split::Split => "split",

        format::uuid::Uuid => "uuid",
        format::ip::Ip => "ip",
        format::cidr::Cidr => "cidr",
        format::hostname::Hostname => "hostname",
        format::base64::Base64 => "base64",
        format::hex::Hex => "hex",
        format::digest::Digest => "digest",
        format::mongo_id::MongoId => "mongo_id",
        format::hex_color::HexColor => "hex_color",
        format::mac::Mac => "mac",
        format::e164::E164 => "e164",
        format::semver::Semver => "semver",
        format::jwt::Jwt => "jwt",

        checksum::luhn::Luhn => "luhn",
        checksum::credit_card::CreditCard => "credit_card",
        checksum::iban::Iban => "iban",
    ]
    ext_metarules: [
        meta::values::Values => "values",
    ]
    // Metarules that also read other fields of the object
    ext_ctx_metarules: [
        meta::when::When => "when",
    ]
}
