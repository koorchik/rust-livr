livr::schema! {
    struct S {
        home: {"nested_object": {city: "string"}} as Addr,
        work: {"nested_object": {city: "string"}} as Addr,
    }
}

livr::schema! {
    struct T {
        home: {"nested_object": {city: "string"}} as T,
    }
}

fn main() {}
