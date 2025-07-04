use crate::{Own, pin};

#[cfg(not(loom))]
compile_error! { r#"test with `RUSTFLAGS="--cfg loom" cargo test`"# }

#[test]
pub fn concurrent_drop_get() {
    loom::model(|| {
        crate::guts::empty_recycler();
        let o = Own::new_box(42);
        let r = o.weak;
        loom::thread::spawn(move || {
            drop(o);
        });
        loom::thread::spawn(move || {
            let g = pin();
            assert!(matches!(r.get(&g), Some(&42) | None));
        });
    });
}

#[test]
pub fn concurrent_reuse_get() {
    loom::model(|| {
        crate::guts::empty_recycler();
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

#[test]
pub fn concurrent_multiple_refs() {
    loom::model(|| {
        crate::guts::empty_recycler();
        let o = Own::new_box(42);
        let r1 = o.weak;
        let r2 = o.weak;
        loom::thread::spawn(move || {
            let g = pin();
            assert!(matches!(r1.get(&g), Some(&42) | None));
        });
        loom::thread::spawn(move || {
            let g = pin();
            assert!(matches!(r2.get(&g), Some(&42) | None));
        });
        loom::thread::spawn(move || {
            drop(o);
        });
    });
}

#[test]
pub fn concurrent_chain_reuse() {
    loom::model(|| {
        crate::guts::empty_recycler();
        let o1 = Own::new_box(1);
        let r1 = o1.weak;
        let o2 = Own::new_from(Box::new(2), o1);
        let r2 = o2.weak;
        loom::thread::spawn(move || {
            let g = pin();
            assert!(matches!(r1.get(&g), None));
            assert!(matches!(r2.get(&g), Some(&2) | None));
        });
        loom::thread::spawn(move || {
            let o3 = Own::new_from(Box::new(3), o2);
            assert_eq!(*o3, 3);
        });
    });
}

#[test]
pub fn concurrent_recycler_stress() {
    loom::model(|| {
        crate::guts::empty_recycler();
        let o1 = Own::new_box(1);
        let r1 = o1.weak;
        let o2 = Own::new_from(Box::new(2), o1);
        let o3 = Own::new_from(Box::new(3), o2);
        let r3 = o3.weak;

        loom::thread::spawn(move || {
            let g = pin();
            assert!(matches!(r1.get(&g), None));
        });
        loom::thread::spawn(move || {
            let g = pin();
            assert!(matches!(r3.get(&g), Some(&3) | None));
        });
        loom::thread::spawn(move || {
            drop(o3);
        });
    });
}
