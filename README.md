# mui

A simple drop guard for [`MaybeUninit`]

Because the inner value of [`MaybeUninit`] (*hereinafter abbreviated as* MUI) never gets dropped unless it is converted to the concrete type (by [`assume_init`](`MaybeUninit::assume_init`)) or manually dropped (by [`assume_init_drop`](`MaybeUninit::assume_init_drop`)),
a memory leak happens and the data on the memory goes out of management if the program panics during initialization (This is the specification of unions).
This crate provides a simple guard type to avoid this problem and reduce some unsafeness.

## Example
### A basic usage
```rust
let data = {
    let mut data = MaybeUninit::<u32>::uninit();

    let mut guard = MuiGuard::new(&mut data);

    guard.write(42);
    guard.finish().unwrap();

    unsafe { data.assume_init() }
};

assert_eq!(
    data,
    42
);
```

### Field-by-field initialization
```rust
#[derive(Debug, PartialEq)]
struct Hoge {
    title: String,
    list: Vec<u32>,
}

let hoge = {
    let mut hoge = MaybeUninit::<Hoge>::uninit();
    let mut guard = MuiGuard::new(&mut hoge);

    let ptr = guard.as_mut_ptr();

    unsafe { (&raw mut (*ptr).title).write("some text".to_string()); }
    unsafe { (&raw mut (*ptr).list).write(vec![810, 114514, 1919]); }

    // Because the guard cannot detect initialization of the value via the pointer,
    // validated finalization would fail regardless of the true state of the MUI.
    guard.finish_unchecked();

    unsafe { hoge.assume_init() }
};

assert_eq!(
    hoge,
    Hoge {
        title: "some text".to_string(),
        list: vec![810, 114514, 1919]
    }
);
```

License: Apache-2.0
