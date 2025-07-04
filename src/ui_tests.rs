use crate::{Guard, Own, Ref, pin};

#[test]
fn live_ref_get_some() {
    let o = Own::new_box(42);
    let r = o.weak;

    let g = pin();
    assert_eq!(r.get(&g), Some(&42));
}

#[test]
fn dead_ref_get_none() {
    let o = Own::new_box(42);
    let r = o.weak;
    drop(o);

    let g = pin();
    assert_eq!(r.get(&g), None);
}

#[test]
fn dead_ref_get_some_with_pin() {
    let o = Own::new_box(42);
    let r = o.weak;

    let g = pin();
    let value = r.get(&g);
    drop(o);
    assert_eq!(value, Some(&42));
}

#[test]
fn dead_ref_get_none_after_reuse() {
    let o = Own::new_box(42);
    let r = o.weak;
    let o = Own::new_from(Box::new(43), o);

    let g = pin();
    assert_eq!(r.get(&g), None);

    let _ = o;
}
