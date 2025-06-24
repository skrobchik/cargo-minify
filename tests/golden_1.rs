fn foo_used() {}

#[allow(dead_code)]
fn main() {
    foo_used();
}
