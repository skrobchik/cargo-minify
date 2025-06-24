#[derive(Default)]
struct AliveStruct {
    x: i32,
}

trait MyTrait {
    fn mytrait_fun() {}
}

struct DeadStruct {}

impl MyTrait for DeadStruct {}

#[allow(dead_code)]
fn main() {
    let s = AliveStruct::default();
    println!("{}", s.x);
}
