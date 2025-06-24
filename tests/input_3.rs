mod foo {
    pub fn foo_used() {}

    pub fn foo_unused() {}
}

#[allow(dead_code)]
fn main() {
    foo::foo_used();
}
