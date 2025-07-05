//! Weakref provides a cheap `Copy + 'static` reference type [`Ref<T>`]. You can
//! pass it anywhere almost effortlessly, then check if the reference is alive
//! at runtime. The single owner [`Own<T>`] increments a global per-object generation
//! counter when dropped.
//!
//! Weakref also provides a <https://docs.rs/owning_ref>-style [`Ref::map`] function.
//!
//! This is inspired by <https://verdagon.dev/blog/surprising-weak-refs>, although
//! the implementation has changed quite a bit vs what is used in Vale.
//!
//! # Basic Usage
//!
//! ```
//! use weakref::{Own, Ref, pin, refer};
//!
//! let data = Own::new(vec![1, 2, 3]);
//!
//! std::thread::spawn(move || {
//!     match refer!(data).get(&pin()) {
//!         Some(data) => println!("{data:?} is still alive!"),
//!         None => println!("data got dropped!"),
//!     }
//! });
//!
//! drop(data);
//! ```
//!
//! # Performance Characteristics
//!
//! Each `Own/Ref` is 24 bytes on the stack, and globally allocates a single 8-byte generation counter. The counter
//! can never be freed (since it must remain accessible to `Ref` forever) but can be reused indefinitely. Access
//! requires pinning the thread with crossbeam_epoch and atomically loading the generation counter to check if
//! it matches. Dropping Own requires pinning the thread, deferring the destructor, incrementing the generation counter,
//! and pushing it to a queue to be reused.
//!
//! Weakref has broadly similar performance as Arc, except with totally free Ref copies. As of version 0.1.0 my benchmarks show Own+Ref behind but with plenty of room still for optimization.
//! |          | Own+Ref | Arc+Weak |
//! | -------- | ------- | -------- |
//! | Creation | 16ns    | 12ns     |
//! | Access   | 5ns     | 3ns      |
//! | Drop     | 60ns    | 20ns     |

use std::path;
use std::pin::Pin;
use std::sync::Arc;
use std::{fmt, ptr::NonNull};

mod guts;
pub use guts::{IsPtr, Own, Ref};

/// A guard that allows continued access to a weakref.
///
/// This is a re-export from [crossbeam_epoch].
pub use crossbeam_epoch::Guard;

/// Prevents weakrefs from being dropped mid-access.
///
/// This is a re-export from [crossbeam_epoch].
pub use crossbeam_epoch::pin;

#[cfg(all(test, loom))]
mod loom_tests;
#[cfg(test)]
mod ui_tests;

impl<T: Send + 'static> Own<Box<T>> {
    /// The standard way to create an `Own<Box<T>> + Ref<T>`.
    ///
    /// This simply allocates a box and wraps it with [Own::new].
    pub fn new_box(value: T) -> Self {
        Self::new(Box::new(value))
    }
}

impl<P: IsPtr + Send> fmt::Debug for Own<P>
where
    P::T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `.field` requires `T: Sized` and `field_with` is unstable
        // f.debug_tuple("Own").field(live).finish()
        write!(f, "Own({:?})", &**self)
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for Ref<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.get(&pin()) {
            Some(live) => {
                // `.field` requires `T: Sized` and `field_with` is unstable
                // f.debug_tuple("Ref::Live").field(live).finish()
                write!(f, "Ref::Live({live:?})")
            }
            None => f.debug_tuple("Ref::Dead").finish_non_exhaustive(),
        }
    }
}

impl<P: IsPtr + core::ops::Deref> IsPtr for Pin<P> {
    type T = P::T;

    fn into_raw_ptr(this: Self) -> NonNull<P::T> {
        // SAFETY: Pin invariant is maintained - we never expose the unpinned T for mutation or move
        let b = unsafe { Pin::into_inner_unchecked(this) };
        IsPtr::into_raw_ptr(b)
    }

    unsafe fn from_raw_ptr(ptr: NonNull<P::T>) -> Self {
        // SAFETY: Pointer must have been returned from into_raw_ptr
        let b: P = unsafe { IsPtr::from_raw_ptr(ptr) };
        // SAFETY: Pin invariant preserved - the T was never exposed for mutation or move during conversion
        unsafe { Pin::new_unchecked(b) }
    }
}

impl<T: ?Sized> IsPtr for Box<T> {
    type T = T;

    fn into_raw_ptr(this: Self) -> NonNull<T> {
        NonNull::new(Box::into_raw(this)).unwrap()
    }

    unsafe fn from_raw_ptr(ptr: NonNull<T>) -> Self {
        // SAFETY: Pointer must have been returned from into_raw_ptr
        unsafe { Box::from_raw(ptr.as_ptr()) }
    }
}

impl<T: ?Sized> IsPtr for Arc<T> {
    type T = T;

    fn into_raw_ptr(this: Self) -> NonNull<T> {
        NonNull::new(Arc::into_raw(this).cast_mut()).unwrap()
    }

    unsafe fn from_raw_ptr(ptr: NonNull<T>) -> Self {
        // SAFETY: Pointer must have been returned from into_raw_ptr
        unsafe { Arc::from_raw(ptr.as_ptr()) }
    }
}

impl IsPtr for String {
    type T = str;

    fn into_raw_ptr(this: Self) -> NonNull<str> {
        let b: Box<str> = this.into();
        IsPtr::into_raw_ptr(b)
    }

    unsafe fn from_raw_ptr(ptr: NonNull<str>) -> Self {
        // SAFETY: Pointer must have been returned from into_raw_ptr
        let b: Box<str> = unsafe { IsPtr::from_raw_ptr(ptr) };
        b.into()
    }
}

impl IsPtr for path::PathBuf {
    type T = path::Path;

    fn into_raw_ptr(this: Self) -> NonNull<path::Path> {
        let b: Box<path::Path> = this.into();
        IsPtr::into_raw_ptr(b)
    }

    unsafe fn from_raw_ptr(ptr: NonNull<path::Path>) -> Self {
        // SAFETY: Pointer must have been returned from into_raw_ptr
        let b: Box<path::Path> = unsafe { IsPtr::from_raw_ptr(ptr) };
        b.into()
    }
}

impl<T> IsPtr for Vec<T> {
    type T = [T];

    fn into_raw_ptr(this: Self) -> NonNull<[T]> {
        let b: Box<[T]> = this.into();
        IsPtr::into_raw_ptr(b)
    }

    unsafe fn from_raw_ptr(ptr: NonNull<[T]>) -> Self {
        // SAFETY: Pointer must have been returned from into_raw_ptr
        let b: Box<[T]> = unsafe { IsPtr::from_raw_ptr(ptr) };
        b.into()
    }
}

impl IsPtr for () {
    type T = ();

    fn into_raw_ptr(_: Self) -> NonNull<Self::T> {
        NonNull::dangling()
    }

    unsafe fn from_raw_ptr(_: NonNull<Self::T>) -> Self {}
}

/// Creates a weak reference without trying to capture the owner.
///
/// This macro is equivalent to [Own::refer] but is better to use
/// inside closures. Calling `owner.refer()` will try to capture
/// a reference to `owner` from the enclosing scope (or otherwise move it entirely).
/// This macro takes advantage of
/// [disjoint capturing](https://doc.rust-lang.org/reference/types/closure.html#capture-precision)
/// to precreate a weak refrence and capture that instead.
///
/// __Make sure to mark your closure as `move`, or you will get a confusing "cannot move out of" error.__
///
/// # Examples
///
/// ```
///# use weakref::{Own, Ref, pin, refer};
///# use std::sync::{Arc, Weak};
/// // With weakref - owner is not moved into closure
/// let data = Own::new_box(42);
/// move || { refer!(data).get(&pin()); };
///
/// // With std - we need to call downgrade outside
/// let data = Arc::new(42);
/// let weak_data = Arc::downgrade(&data);
/// move || { weak_data.upgrade(); };
/// ```
#[macro_export]
macro_rules! refer {
    ($owner:expr) => {{
        let r: $crate::Ref<_> = $owner._weak;
        r
    }};
}
