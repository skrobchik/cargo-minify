fn foo_used() {}

fn foo_unused() {}

#[allow(dead_code)]
fn main() {
    foo_used();
}
