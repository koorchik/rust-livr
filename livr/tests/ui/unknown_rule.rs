livr::schema! {
    struct User {
        name: "required",
        email: ["required", "emial"],
    }
}

fn main() {}
