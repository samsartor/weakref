use crate::Own;
use crate::guts::{empty_recycler, local_recycler_len, global_recycler_len};

#[test]
fn recycler_starts_empty() {
    empty_recycler();
    assert_eq!(local_recycler_len(), 0);
    assert_eq!(global_recycler_len(), 0);
}

#[test]
fn recycler_populates_local_on_first_allocation() {
    empty_recycler();
    
    let o = Own::new_box(42);
    drop(o);
    
    assert!(local_recycler_len() > 0);
    assert_eq!(global_recycler_len(), 0);
}

#[test]
fn recycler_moves_to_global_when_local_full() {
    empty_recycler();
    
    let mut objects = Vec::new();
    for i in 0..1536 {
        objects.push(Own::new_box(i));
    }
    
    for obj in objects {
        drop(obj);
    }
    
    assert!(global_recycler_len() > 0);
}

#[test]
fn recycler_reuses_from_local_first() {
    empty_recycler();
    
    let o1 = Own::new_box(42);
    drop(o1);
    
    let initial_local_len = local_recycler_len();
    assert!(initial_local_len > 0);
    
    let _o2 = Own::new_box(43);
    
    assert_eq!(local_recycler_len(), initial_local_len - 1);
}

#[test]
fn recycler_pulls_from_global_when_local_empty() {
    empty_recycler();
    
    let mut objects = Vec::new();
    for i in 0..1536 {
        objects.push(Own::new_box(i));
    }
    
    for obj in objects {
        drop(obj);
    }
    
    let initial_global_len = global_recycler_len();
    assert!(initial_global_len > 0);
    
    empty_recycler();
    
    let o = Own::new_box(42);
    drop(o);
    
    assert!(local_recycler_len() > 0);
}