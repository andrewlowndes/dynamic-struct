# Dynamic struct
A derive macro for creating **push-based** reactive properties for structs (with named fields only).

## Why push-based?
Lazy *poll-based* reactive systems typically require wrapping the values and adding RefCells or flags to cache and update values. Event-based system require a subscription model.

The plumbing for adding *push-based* change propagation is done via macros at compile-time and the generated code can be inlined during compilation, becoming a zero-cost abstraction at run-time (same as re-calculating the dynamic properties by hand when their dependencies change)

The types can also be left untouched, no need for wrapping and dereferencing.

## How to use
**1)** Add as a dependency to the Cargo file
```toml
[dependencies]
dynamic-struct = "0.1"
```

**2)** Add the derive macro to the struct and mark the properties that are dynamic
```rust
use dynamic_struct::Dynamic;

#[derive(Dynamic)]
struct Demo {
    a: u32,
    b: u32,
    #[dynamic((a, b), calculate_c)]
    c: u32,
}

impl Demo {
    fn calculate_c(&mut self) {
        self.c = self.a + self.b
    }
}
```

The attribute for the properties has the following structure:
```rust
#[dynamic(tuple of dependent property names, name of local method name)]
```

The local method must have the call signature matching `fn name(&mut self)`.

**3)** Update the properties using the generated mutate functions
```rust
fn main() {
    let demo = Demo { a: 1, b: 2, c: 3 };

    dbg!(demo.c); //3
    demo.update_a(7);
    dbg!(demo.c); //9
}
```

## How it works

**1)** Functions are created to signal when a property is changed, it is populated with the methods that should be called.

```rust
impl Demo {
    #[inline]
    pub fn updated_a(&mut self) {
        self.update_c();
    }
}
```

Note: properties that do not propagate changes will still be created but will be empty.

**2)** Functions are created for each property to update the property

For **non-dynamic** properties, the value can be set via a parameter matching the field type, then the field updated function is called (listed above).

```rust
impl Demo {
    #[inline]
    pub fn update_a(&mut self, a: u32) {
        self.a = a;
        self.updated_a();
    }
}
```

For **dynamic** properties, the value is set by calling the specified dynamic function, then the field updated function is called (listed above).

```rust
impl Demo {
    #[inline]
    pub fn update_c(&mut self) {
        self.calculate_c();
        self.updated_c();
    }
}
```

Note: be careful not to create cyclic dependencies!

## Configuration

The names of the generated functions can be customised by declaring a struct attribute and overriding a prefix/suffix. e.g:

```Rust
#[derive(Dynamic)]
#[dynamic(setter_prefix = "set_", setter_suffix = "_value")]
struct MyStruct {
    a: u32,
    b: u32,
}

fn main() {
    let test = MyStruct { a: 1, b: 2 };

    test.set_a_value(3);
    test.set_b_value(4);
}
```

Properties that can specified include:

| Name | Type | Comment |
| - | - | - |
| updated_prefix | str | Prefix for updated methods |
| updated_suffix | str | Suffix for updated methods  |
| setter_prefix | str | Prefix for setter methods (non-dynamic fields) |
| setter_suffix | str | Suffix for setter methods (non-dynamic fields) |
| update_prefix | str | Prefix for update methods (dynamic fields) |
| update_suffix | str | Suffix for update methods (dynamic fields) |
