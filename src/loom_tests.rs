use crate::{Own, pin, refer};

#[cfg(not(loom))]
compile_error! { r#"test with `RUSTFLAGS="--cfg loom" cargo test`"# }

#[test]
pub fn concurrent_drop_get() {
    loom::model(|| {
        crate::guts::empty_recycler();
        let o = Own::new_box(42);
        let r = o.refer();
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
            assert!(matches!(refer!(o).get(&g), Some(&42) | None));
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
        let r1 = o.refer();
        let r2 = o.refer();
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
        let r1 = o1.refer();
        let o2 = Own::new_from(Box::new(2), o1);
        let r2 = o2.refer();
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
        let r1 = o1.refer();
        let o2 = Own::new_from(Box::new(2), o1);
        let o3 = Own::new_from(Box::new(3), o2);
        let r3 = o3.refer();

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

/*
#[test]
pub fn concurrent_replace_with_bad() {
    use std::mem::ManuallyDrop;
    use std::panic::catch_unwind;
    loom::model(|| {
        crate::guts::empty_recycler();
        let mut orig = ManuallyDrop::new(Own::new_box(42));
        let mut dupl = ManuallyDrop::new(Own::new_box(0));
        dupl.refer() = orig.refer();
        loom::thread::spawn(move || {
            // panicing is good behavior
            // the unsafe is not needed, just lets us test better
            let _ = catch_unwind(move || unsafe { ManuallyDrop::drop(&mut dupl) });
        });
        loom::thread::spawn(move || {
            // USE AFTER FREE!
            println!("{}", &**orig);
            // panicing is good behavior
            // the unsafe is not needed, just lets us test better
            let _ = catch_unwind(move || unsafe { ManuallyDrop::drop(&mut orig) });
        });
    });
}
*/
