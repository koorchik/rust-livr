livr::schema! {
    #[alias(a = ["required", "b"])]
    #[alias(b = {"list_of": "a"})]
    struct User {
        name: "a",
    }
}

fn main() {}
