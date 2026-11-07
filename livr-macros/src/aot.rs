//! The code of the rules of a field, compiled ahead of time: each built-in
//! rule is called directly, its arguments are constants when they can be.
//! Rules with heap data are built once from their arguments, the others
//! (metarules inside a chain, aliases, custom rules) are compiled by the
//! engine once.

use livr_core::util::number_arg;
use proc_macro2::{Literal, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use serde_json::Value;
use syn::Ident;

use crate::Gen;
use crate::infer::{rule_list, rule_parts};
use crate::names::pascal_case;

impl Gen<'_> {
    /// A function that runs the rules on a value, like a `Chain` does.
    pub(crate) fn rules_fn(&self, ident: &Ident, rules: &Value) -> TokenStream2 {
        let steps = rule_list(rules).iter().map(|rule| self.step(rule));
        quote! {
            #[inline]
            #[allow(unused_variables)]
            fn #ident<'v>(
                value: ::std::borrow::Cow<'v, ::livr::Value>,
                ctx: &::livr::Ctx<'_>,
            ) -> ::livr::Checked<'v> {
                #(let value = #steps?;)*
                ::core::result::Result::Ok(value)
            }
        }
    }

    /// Whether a rule reads other fields of the object, like
    /// `Chain::reads_ctx`: aliases and `or` see the object of the field, so
    /// they read it when their rules do.
    pub(crate) fn reads_ctx(&self, rules: &Value) -> bool {
        self.reads_ctx_in(rules, &mut Vec::new())
    }

    fn reads_ctx_in<'r>(&'r self, rules: &'r Value, aliases: &mut Vec<&'r str>) -> bool {
        rule_list(rules).iter().any(|rule| {
            let Some((name, args)) = rule_parts(rule) else {
                return false;
            };
            if self.infer.is_custom(name) {
                return true;
            }
            if let Some(alias_rules) = self.infer.alias_rules(name) {
                // A recursive alias is a schema error of the engine
                if aliases.contains(&name) {
                    return false;
                }
                aliases.push(name);
                let reads_ctx = self.reads_ctx_in(alias_rules, aliases);
                aliases.pop();
                return reads_ctx;
            }
            match self.infer.builtin_name(name) {
                Some("equal_to_field" | "required_if" | "when") => true,
                Some("or") => args.iter().any(|rules| self.reads_ctx_in(rules, aliases)),
                _ => false,
            }
        })
    }

    // An expression that runs the rule on `value`: a `Checked`
    fn step(&self, rule: &Value) -> TokenStream2 {
        let (name, args) = rule_parts(rule).expect("rule checked by the engine");
        let Some(builtin) = self.infer.builtin_name(name) else {
            return self.compiled(rule);
        };
        // The names that would shadow the prelude (`String`, `Eq`,
        // `Default`) have a `Rule` suffix
        let ty = match builtin {
            "string" | "eq" | "default" => format_ident!("{}Rule", pascal_case(builtin)),
            builtin => format_ident!("{}", pascal_case(builtin)),
        };
        let constant = |rule: TokenStream2| {
            quote! {{
                const RULE: ::livr::rules::#ty = #rule;
                ::livr::__private::step(&RULE, value, ctx)
            }}
        };
        // Number arguments are checked by the engine; they are finite, unless
        // a string has hundreds of digits
        let number = |index| {
            number_arg(args, index, name)
                .ok()
                .filter(|number| number.is_finite())
                .map(Literal::f64_suffixed)
        };
        match builtin {
            "required" | "not_empty" | "not_empty_list" | "any_object" | "string" | "integer"
            | "positive_integer" | "decimal" | "positive_decimal" | "email" | "url" | "trim"
            | "to_lc" | "to_uc" | "boolean" | "any" | "digits" | "hostname" | "hex"
            | "mongo_id" | "hex_color" | "e164" | "semver" | "jwt" | "luhn" | "credit_card"
            | "iban" => constant(quote! { ::livr::rules::#ty }),
            // Without arguments, the rule of LIVR 2.0; an object of bounds
            // (LIVR 2.5) is built (any other first argument is a schema
            // error)
            "iso_date" if !args.first().is_some_and(Value::is_object) => {
                constant(quote! { ::livr::rules::#ty::new() })
            }
            "max_length" | "min_length" | "length_equal" | "max_number" | "min_number" => {
                match number(0) {
                    Some(number) => constant(quote! { ::livr::rules::#ty::new(#number) }),
                    None => self.built(&ty, args),
                }
            }
            "length_between" | "number_between" => match (number(0), number(1)) {
                (Some(min), Some(max)) => constant(quote! { ::livr::rules::#ty::new(#min, #max) }),
                _ => self.built(&ty, args),
            },
            "nested_object"
            | "variable_object"
            | "list_of"
            | "list_of_objects"
            | "list_of_different_objects"
            | "when"
            | "values"
            | "keys" => self.compiled(rule),
            _ => self.built(&ty, args),
        }
    }

    // A rule built from its arguments on first use
    fn built(&self, ty: &Ident, args: &[Value]) -> TokenStream2 {
        let main = self.main;
        let args = serde_json::to_string(args).expect("JSON");
        quote! {{
            static RULE: ::livr::__private::LazyLock<::livr::rules::#ty> =
                ::livr::__private::LazyLock::new(|| {
                    // The registry is built only if the rule compiles
                    // rules (`or`): it is costly on a cold start
                    ::livr::__private::build(#main::__livr_registry, #main::__LIVR_CLOCK, #args, ::livr::rules::#ty::build)
                });
            ::livr::__private::step(&*RULE, value, ctx)
        }}
    }

    // A rule compiled by the engine, on first use
    fn compiled(&self, rule: &Value) -> TokenStream2 {
        let main = self.main;
        let rule = serde_json::to_string(rule).expect("JSON");
        quote! {{
            static RULES: ::livr::__private::LazyLock<::livr::Chain> =
                ::livr::__private::LazyLock::new(|| {
                    ::livr::__private::chain(#main::__livr_registry(), #main::__LIVR_CLOCK, #rule)
                });
            RULES.run(value, ctx)
        }}
    }
}
