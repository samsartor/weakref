use crate::{Own, pin};
use std::sync::Arc;

#[test]
fn live_ref_get_some() {
    let o = Own::new_box(42);
    let r = o.refer();

    let g = pin();
    assert_eq!(r.get(&g), Some(&42));
}

#[test]
fn dead_ref_get_none() {
    let o = Own::new_box(42);
    let r = o.refer();
    drop(o);

    let g = pin();
    assert_eq!(r.get(&g), None);
}

#[test]
fn dead_ref_get_some_with_pin() {
    let o = Own::new_box(42);
    let r = o.refer();

    let g = pin();
    let value = r.get(&g);
    drop(o);
    assert_eq!(value, Some(&42));
}

#[test]
fn dead_ref_get_none_after_reuse() {
    let o = Own::new_box(42);
    let r = o.refer();
    let o = Own::new_from(Box::new(43), o);

    let g = pin();
    assert_eq!(r.get(&g), None);

    let _ = o;
}

#[test]
fn ref_with_helper() {
    let o = Own::new_box(42);
    let r = o.refer();

    let result = r.inspect(|x| *x * 2);
    assert_eq!(result, Some(84));

    drop(o);
    let result = r.inspect(|x| *x * 2);
    assert_eq!(result, None);
}

#[test]
fn ref_map_helper() {
    let o = Own::new_box(String::from("hello"));
    let r = o.refer();

    let mapped = r.map(|s| s.as_str());
    let g = pin();
    assert_eq!(mapped.get(&g), Some("hello"));

    drop(o);
    assert_eq!(mapped.get(&g), None);
}

#[test]
fn ref_map_with_guard() {
    let o = Own::new_box(vec![1, 2, 3]);
    let r = o.refer();

    let g = pin();
    let mapped = r.map_with(|v| &v[1], &g);
    assert_eq!(mapped.get(&g), Some(&2));

    drop(o);
    assert_eq!(mapped.get(&g), None);
}

#[test]
fn ref_copy_and_clone() {
    let o = Own::new_box(42);
    let r1 = o.refer();
    let r2 = r1;
    #[allow(clippy::clone_on_copy)]
    let r3 = r1.clone();

    let g = pin();
    assert_eq!(r1.get(&g), Some(&42));
    assert_eq!(r2.get(&g), Some(&42));
    assert_eq!(r3.get(&g), Some(&42));

    drop(o);
    assert_eq!(r1.get(&g), None);
    assert_eq!(r2.get(&g), None);
    assert_eq!(r3.get(&g), None);
}

#[test]
fn multiple_refs_same_object() {
    let o = Own::new_box(42);
    let r1 = o.refer();
    let r2 = o.refer();

    let g = pin();
    assert_eq!(r1.get(&g), Some(&42));
    assert_eq!(r2.get(&g), Some(&42));

    drop(o);
    assert_eq!(r1.get(&g), None);
    assert_eq!(r2.get(&g), None);
}

#[test]
fn arc_pointer_type() {
    let o = Own::new(Arc::new(42));
    let r = o.refer();

    let g = pin();
    assert_eq!(r.get(&g), Some(&42));

    drop(o);
    assert_eq!(r.get(&g), None);
}

#[test]
fn string_pointer_type() {
    let o = Own::new(String::from("hello"));
    let r = o.refer();

    let g = pin();
    assert_eq!(r.get(&g), Some("hello"));

    drop(o);
    assert_eq!(r.get(&g), None);
}

#[test]
fn vec_pointer_type() {
    let o = Own::new(vec![1, 2, 3]);
    let r = o.refer();

    let g = pin();
    assert_eq!(r.get(&g), Some(&[1, 2, 3][..]));

    drop(o);
    assert_eq!(r.get(&g), None);
}

#[test]
fn unit_pointer_type() {
    let o = Own::new(());
    let r = o.refer();

    let g = pin();
    assert_eq!(r.get(&g), Some(&()));

    drop(o);
    assert_eq!(r.get(&g), None);
}

#[test]
fn sequential_reuse() {
    let o1 = Own::new_box(1);
    let r1 = o1.refer();
    let o2 = Own::new_from(Box::new(2), o1);
    let r2 = o2.refer();
    let o3 = Own::new_from(Box::new(3), o2);

    let g = pin();
    assert_eq!(r1.get(&g), None);
    assert_eq!(r2.get(&g), None);
    assert_eq!(o3.refer().get(&g), Some(&3));

    drop(o3);
    assert_eq!(r1.get(&g), None);
    assert_eq!(r2.get(&g), None);
}

#[test]
fn debug_formatting() {
    let o = Own::new_box(42);
    let r = o.refer();

    let debug_str = format!("{o:?}");
    assert!(debug_str.contains("Own"));
    assert!(debug_str.contains("42"));

    let debug_str = format!("{r:?}");
    assert!(debug_str.contains("Ref::Live"));
    assert!(debug_str.contains("42"));

    drop(o);
    let debug_str = format!("{r:?}");
    assert!(debug_str.contains("Ref::Dead"));
}

#[test]
fn deref_trait() {
    let o = Own::new_box(42);
    assert_eq!(*o, 42);

    let s = Own::new(String::from("hello"));
    assert_eq!(&*s, "hello");
}

/*
#[test]
#[should_panic]
fn replace_weak_with_valid() {
    let a = Own::new_box(42);
    let mut b = Own::new_box(43);
    b.weak = a.refer();
    drop(a);
    drop(b);
}

#[test]
#[should_panic]
#[cfg(not(loom))]
fn replace_weak_with_null() {
    let mut o = Own::new_box(42);
    o.weak = Ref::null();
    dbg!(&*o);
}
*/
