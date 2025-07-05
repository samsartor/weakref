//! Weakref provides a cheep `Copy + 'static` reference type [`Ref<T>`]. You can
//! pass it anywhere almost effortlessly, then check if the reference is alive
//! at runtime.
//!
//! ```
//! use weakref::{Own, Ref, pin};
//!
//! let data = Own::new(vec![1, 2, 3]);
//!
//! std::thread::spawn(move || {
//!     match data.weak.get(&pin()) {
//!         Some(data) => println!("{data:?} is still alive!"),
//!         None => println!("data got dropped!"),
//!     }
//! });
//!
//! drop(data);
//! ```
//!
//! Notice you can downgrade an `owner: Own<Box<T>>` to a weak
//! `Ref<T>` by simply accessing [owner.weak](field@Own::weak)
//! field. This allows closures to [capture weak references from owners](https://doc.rust-lang.org/reference/types/closure.html#capture-precision)
//! without any need to pre-call methods like `Arc.clone()`:
//! ```
//!# use weakref::{Own, Ref, pin};
//!# use std::sync::{Arc, Weak};
//! // With weakref
//! let data = Own::new_box(42);
//! move || { data.weak.get(&pin()); };
//!
//! // With std
//! let data = Arc::new(42);
//! let weak_data = Arc::downgrade(&data);
//! move || { weak_data.upgrade(); };
//! ```

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
        // SAFTEY: we never expose the unpinned T for mutation or move
        let b = unsafe { Pin::into_inner_unchecked(this) };
        IsPtr::into_raw_ptr(b)
    }

    unsafe fn from_raw_ptr(ptr: NonNull<P::T>) -> Self {
        // SAFETY: same guarentees as the caller
        let b: P = unsafe { IsPtr::from_raw_ptr(ptr) };
        // SAFTEY: we never exposed the unpinned T for mutation or move
        unsafe { Pin::new_unchecked(b) }
    }
}

impl<T: ?Sized> IsPtr for Box<T> {
    type T = T;

    fn into_raw_ptr(this: Self) -> NonNull<T> {
        NonNull::new(Box::into_raw(this)).unwrap()
    }

    unsafe fn from_raw_ptr(ptr: NonNull<T>) -> Self {
        // SAFETY: same guarentees as the caller
        unsafe { Box::from_raw(ptr.as_ptr()) }
    }
}

impl<T: ?Sized> IsPtr for Arc<T> {
    type T = T;

    fn into_raw_ptr(this: Self) -> NonNull<T> {
        NonNull::new(Arc::into_raw(this).cast_mut()).unwrap()
    }

    unsafe fn from_raw_ptr(ptr: NonNull<T>) -> Self {
        // SAFETY: same guarentees as the caller
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
        // SAFETY: same guarentees as the caller
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
        // SAFETY: same guarentees as the caller
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
        // SAFETY: same guarentees as the caller
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
