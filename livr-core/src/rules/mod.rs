//! The built-in rules of the LIVR specification, one per file. Their checks
//! are public, so typed code can call them directly.

pub mod common;
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
    pub(crate) use crate::rule::{Ctx, Outcome};
    pub(crate) use crate::util::{
        Num, arg, char_len, is_no_value, is_primitive, js_trim, number_arg, parse_number,
        string_arg, to_js_string,
    };
    pub(crate) use serde_json::Value;
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
    ) => {
        $(pub use $group::$file::$rule;)*
        $(pub use $meta_group::$meta_file::$meta;)*

        /// A compiled rule: built-in rules are dispatched without virtual
        /// calls.
        pub(crate) enum CompiledRule {
            $($rule($rule),)*
            $($meta($meta),)*
            Alias(AliasRule),
            Custom(Box<dyn Rule>),
        }

        impl std::fmt::Debug for CompiledRule {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$rule(rule) => std::fmt::Debug::fmt(rule, f),)*
                    $(Self::$meta(rule) => std::fmt::Debug::fmt(rule, f),)*
                    Self::Alias(rule) => std::fmt::Debug::fmt(rule, f),
                    // A custom rule can be any type
                    Self::Custom(_) => f.write_str("Custom(..)"),
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
                    Self::Alias(rule) => rule.run(value),
                    Self::Custom(rule) => {
                        let outcome = rule.check(&value, ctx);
                        checked(outcome, value)
                    }
                }
            }

            /// Whether the rule reads other fields of the object.
            pub(crate) fn reads_ctx(&self) -> bool {
                matches!(self, Self::EqualToField(_) | Self::Custom(_))
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

        /// The built-in rules by their snake_case names.
        pub(crate) const BUILT_IN_RULES: &[(&str, BuildFn)] = &[
            $(($name, |args, ctx| Ok(CompiledRule::$rule($rule::build(args, ctx)?))),)*
            $(($meta_name, |args, ctx| Ok(CompiledRule::$meta($meta::build(args, ctx)?))),)*
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
}
