use dynamic_struct::Dynamic;

#[derive(Default, Dynamic)]
struct Demo {
    a: u32,
    b: u32,
    #[dynamic((a, b), calc_c)]
    c: u32,
    #[dynamic((c), calc_d)]
    d: u32,
}

impl Demo {
    pub fn new(a: u32, b: u32) -> Self {
        let mut instance = Self {
            a,
            b,
            ..Self::default()
        };
        instance.calc_c();
        instance.calc_d();
        instance
    }

    fn calc_c(&mut self) {
        println!("calculating c...");
        self.c = self.a + self.b
    }

    fn calc_d(&mut self) {
        println!("calculating d...");
        self.d = self.c + self.c
    }
}

fn main() {
    let mut demo = Demo::new(1, 2);

    //note: the calculating calls appear before the reading calls
    let Demo { c, d, .. } = &demo;
    println!("c: {c}");
    println!("d: {d}");

    println!("updating a...");
    demo.update_a(4);

    let Demo { c, d, .. } = &demo;
    println!("c: {c}");
    println!("d: {d}");

    println!("updating a...");
    demo.update_a(4);
    println!("updating b...");
    demo.update_b(9);

    let Demo { c, d, .. } = &demo;
    println!("c: {c}");
    println!("d: {d}");
}
