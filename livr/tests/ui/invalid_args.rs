livr::schema! {
    struct User {
        name: {"max_length": "long"},
        zip: {"like": "(?<=x)"},
    }
}

fn main() {}
