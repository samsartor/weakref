# weakref

Weakref provides a cheap `Copy + 'static` reference type [`Ref<T>`]. You can
pass it anywhere almost effortlessly, then check if the reference is alive
at runtime.

This is inspired by <https://verdagon.dev/blog/surprising-weak-refs>, although
the implementation has changed quite a bit vs what is used in Vale.

## Basic Usage

```rust
use weakref::{Own, Ref, pin, refer};

let data = Own::new(vec![1, 2, 3]);

std::thread::spawn(move || {
    match refer!(data).get(&pin()) {
        Some(data) => println!("{data:?} is still alive!"),
        None => println!("data got dropped!"),
    }
});

drop(data);
```

## Performance Characteristics

- **Creation**: `Own::new()` is O(1) with minimal allocation overhead
- **Copying refs**: `Ref` is `Copy`, so totally free
- **Access**: `Ref::get()` is O(1) but requires acquiring an epoch guard
- **Dropping**: `Own` drop is O(1), cleanup is deferred to epoch collection
- **Memory**: Each `Own` and `Ref` has ~24 bytes overhead (in addition to the original pointer and data)

Note that weakref must leak ~8 bytes for every simultaneously-existing object. Those leaked allocations
will be reused by weakref indefinitely but can never be returned to the system.

Compared to `Arc<T>` + `Weak<T>`:
- Faster cloning (no atomic operations)
- Cheaper storage (no reference counting)
- Requires explicit pinning for access
- Uses epoch-based memory reclamation instead of reference counting

License: MIT
