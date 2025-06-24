mod mymod {
    pub struct MyStruct {}

    impl MyStruct {
        pub fn new() -> Self {
            Self {}
        }
    }

    pub fn myfun() -> MyStruct {
        MyStruct::new()
    }
}

#[allow(dead_code)]
fn main() {}
