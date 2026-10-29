livr::schema! {
    struct S {
        a: {"nested_object": {b: "integer", b: "string"}},
    }
}

fn main() {}
