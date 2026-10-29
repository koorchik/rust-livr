use livr::{CompileCtx, Outcome, Rule, SchemaError, Value, rule_fn};

fn int_rule(_args: &[Value], _ctx: &mut CompileCtx<'_>) -> Result<impl Rule + use<>, SchemaError> {
    Ok(rule_fn(|_, _| Outcome::Valid))
}

livr::schema! {
    #[rule(num = int_rule, output = i64)]
    #[alias(num = ["string"])]
    struct Same {
        n: "num",
    }
}

livr::schema! {
    #[rule(foo_bar = int_rule, output = i64)]
    #[alias(fooBar = ["string"])]
    struct CamelCase {
        n: "fooBar",
    }
}

fn main() {}
