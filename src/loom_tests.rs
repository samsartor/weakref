use crate::{Guard, Own, Ref, pin};

#[cfg(not(loom))]
compile_error! { r#"test with `RUSTFLAGS="--cfg loom" cargo test`"# }

#[test]
pub fn concurrent_reuse_get() {
    loom::model(|| {
        let o = Own::new_box(42);
        loom::thread::spawn(move || {
            let g = pin();
            assert!(matches!(o.weak.get(&g), Some(&42) | None));
        });
        loom::thread::spawn(move || {
            let o2 = Own::new_from(Box::new(43), o);
            assert_eq!(*o2, 43);
        });
    });
}
