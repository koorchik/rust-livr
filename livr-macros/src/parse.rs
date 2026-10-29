//! The input of `schema!`: a struct with LIVR rules as its fields, in a JSON
//! syntax (keys of objects can be identifiers), or the path of a JSON file.

use proc_macro2::Span;
use serde_json::{Map, Number, Value};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, Ident, LitBool, LitFloat, LitInt, LitStr, Token, Visibility, braced, bracketed,
};

pub struct SchemaInput {
    // Attributes for the generated types, like #[derive(Debug)]
    pub attrs: Vec<Attribute>,
    pub rules: Vec<CustomRule>,
    pub aliases: Vec<AliasAttr>,
    pub vis: Visibility,
    pub name: Ident,
    pub body: Body,
}

pub enum Body {
    Fields(Vec<FieldInput>),
    File(LitStr),
}

pub struct FieldInput {
    pub key: String,
    pub span: Span,
    pub rules: Value,
    // `as Name`: the name of the type generated for the field
    pub type_name: Option<Ident>,
}

/// `#[rule(uuid = my_rules::uuid, output = String, present)]`
pub struct CustomRule {
    pub name: String,
    pub span: Span,
    pub builder: syn::Path,
    // None: the type of the value does not change
    pub output: Option<syn::Type>,
    pub present: bool,
}

/// `#[alias(adult_age = ["positive_integer", {"min_number": 18}], error = "WRONG_AGE")]`
pub struct AliasAttr {
    pub name: String,
    pub span: Span,
    pub rules: Value,
    pub error: Option<String>,
}

impl Parse for SchemaInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut attrs = Vec::new();
        let mut rules = Vec::new();
        let mut aliases = Vec::new();
        for attr in input.call(Attribute::parse_outer)? {
            if attr.path().is_ident("rule") {
                rules.push(attr.parse_args_with(parse_custom_rule)?);
            } else if attr.path().is_ident("alias") {
                aliases.push(attr.parse_args_with(parse_alias)?);
            } else {
                attrs.push(attr);
            }
        }
        let vis: Visibility = input.parse()?;
        input.parse::<Token![struct]>()?;
        let name: Ident = input.parse()?;

        let body = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            let path: LitStr = input.parse()?;
            input.parse::<Option<Token![;]>>()?;
            Body::File(path)
        } else {
            let content;
            braced!(content in input);
            let mut fields: Vec<FieldInput> = Vec::new();
            while !content.is_empty() {
                let (key, span) = parse_key(&content)?;
                // The schema is a JSON object, where the last of two equal
                // keys wins: a second field would be another field in Rust
                if fields.iter().any(|field| field.key == key) {
                    return Err(syn::Error::new(span, format!("duplicate field `{key}`")));
                }
                content.parse::<Token![:]>()?;
                let rules = parse_json(&content)?;
                let type_name = if content.peek(Token![as]) {
                    content.parse::<Token![as]>()?;
                    Some(content.parse()?)
                } else {
                    None
                };
                fields.push(FieldInput {
                    key,
                    span,
                    rules,
                    type_name,
                });
                if content.is_empty() {
                    break;
                }
                content.parse::<Token![,]>()?;
            }
            Body::Fields(fields)
        };
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after the schema"));
        }
        Ok(Self {
            attrs,
            rules,
            aliases,
            vis,
            name,
            body,
        })
    }
}

fn parse_custom_rule(input: ParseStream<'_>) -> syn::Result<CustomRule> {
    let name = Ident::parse_any(input)?;
    input.parse::<Token![=]>()?;
    let builder: syn::Path = input.parse()?;
    let mut output = None;
    let mut present = false;
    while input.parse::<Option<Token![,]>>()?.is_some() && !input.is_empty() {
        let option = Ident::parse_any(input)?;
        if option == "output" {
            input.parse::<Token![=]>()?;
            output = Some(input.parse()?);
        } else if option == "present" {
            present = true;
        } else {
            return Err(syn::Error::new(
                option.span(),
                "expected `output = Type` or `present`",
            ));
        }
    }
    Ok(CustomRule {
        name: name.unraw().to_string(),
        span: name.span(),
        builder,
        output,
        present,
    })
}

fn parse_alias(input: ParseStream<'_>) -> syn::Result<AliasAttr> {
    let name = Ident::parse_any(input)?;
    input.parse::<Token![=]>()?;
    let rules = parse_json(input)?;
    let mut error = None;
    while input.parse::<Option<Token![,]>>()?.is_some() && !input.is_empty() {
        let option = Ident::parse_any(input)?;
        if option != "error" {
            return Err(syn::Error::new(
                option.span(),
                "expected `error = \"CODE\"`",
            ));
        }
        input.parse::<Token![=]>()?;
        error = Some(input.parse::<LitStr>()?.value());
    }
    Ok(AliasAttr {
        name: name.unraw().to_string(),
        span: name.span(),
        rules,
        error,
    })
}

// A field name or a key of an object: an identifier or a string
fn parse_key(input: ParseStream<'_>) -> syn::Result<(String, Span)> {
    if input.peek(LitStr) {
        let key: LitStr = input.parse()?;
        Ok((key.value(), key.span()))
    } else {
        let key = Ident::parse_any(input)?;
        Ok((key.unraw().to_string(), key.span()))
    }
}

/// A JSON value: strings, numbers, `true`, `false`, `null`, lists and objects.
pub fn parse_json(input: ParseStream<'_>) -> syn::Result<Value> {
    if input.peek(LitStr) {
        return Ok(Value::String(input.parse::<LitStr>()?.value()));
    }
    if input.peek(LitBool) {
        return Ok(Value::Bool(input.parse::<LitBool>()?.value));
    }
    if input.peek(Token![-]) {
        input.parse::<Token![-]>()?;
        return parse_number(input, true);
    }
    if input.peek(LitInt) || input.peek(LitFloat) {
        return parse_number(input, false);
    }
    if input.peek(syn::token::Bracket) {
        let content;
        bracketed!(content in input);
        let mut items = Vec::new();
        while !content.is_empty() {
            items.push(parse_json(&content)?);
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }
        return Ok(Value::Array(items));
    }
    if input.peek(syn::token::Brace) {
        let content;
        braced!(content in input);
        let mut fields = Map::new();
        while !content.is_empty() {
            let (key, span) = parse_key(&content)?;
            // In JSON the last of two equal keys wins, silently: here it is
            // most likely a mistake
            if fields.contains_key(&key) {
                return Err(syn::Error::new(span, format!("duplicate key `{key}`")));
            }
            content.parse::<Token![:]>()?;
            fields.insert(key, parse_json(&content)?);
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }
        return Ok(Value::Object(fields));
    }
    if input.peek(Ident) {
        let ident: Ident = input.parse()?;
        if ident == "null" {
            return Ok(Value::Null);
        }
        return Err(syn::Error::new(
            ident.span(),
            "expected a JSON value: a rule name is a string, like \"required\"",
        ));
    }
    Err(input.error("expected a JSON value"))
}

fn parse_number(input: ParseStream<'_>, is_negative: bool) -> syn::Result<Value> {
    let sign = if is_negative { "-" } else { "" };
    let (digits, span) = if input.peek(LitInt) {
        let int: LitInt = input.parse()?;
        if !int.suffix().is_empty() {
            return Err(syn::Error::new(
                int.span(),
                "numbers have no suffix in JSON",
            ));
        }
        let digits = format!("{sign}{}", int.base10_digits());
        if let Ok(int) = digits.parse::<i64>() {
            return Ok(Value::Number(int.into()));
        }
        (digits, int.span())
    } else {
        let float: LitFloat = input.parse()?;
        (format!("{sign}{}", float.base10_digits()), float.span())
    };
    digits
        .parse::<f64>()
        .ok()
        .and_then(Number::from_f64)
        .map(Value::Number)
        .ok_or_else(|| syn::Error::new(span, "invalid number"))
}
