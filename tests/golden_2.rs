#[derive(Default)]
struct AliveStruct {
    x: i32,
}

#[allow(dead_code)]
fn main() {
    let s = AliveStruct::default();
    println!("{}", s.x);
}
