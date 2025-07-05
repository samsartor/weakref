# weakref

Weakref provides a cheap `Copy + 'static` reference type `Ref<T>`. You can
pass it anywhere almost effortlessly, then check if the reference is alive
at runtime. The single owner `Own<T>` increments a global per-object generation
counter when dropped.

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

Each `Own/Ref` is 24 bytes on the stack, and globally allocates a single 8-byte generation counter. The counter
can never be freed (since it must remain accessible to `Ref` forever) but can be reused indefinitely. Access
requires pinning the thread with crossbeam_epoch and atomically loading the generation counter to check if
it matches. Dropping Own requires pinning the thread, deferring the destructor, incrementing the generation counter,
and pushing it to a queue to be reused.

Weakref has broadly similar performance as Arc, except with totally free Ref copies. As of version 0.1.0 my benchmarks show Own+Ref behind but with plenty of room still for optimization.
|          | Own+Ref | Arc+Weak |
| -------- | ------- | -------- |
| Creation | 16ns    | 12ns     |
| Access   | 5ns     | 3ns      |
| Drop     | 60ns    | 20ns     |

