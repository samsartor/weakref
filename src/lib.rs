use std::rc::Rc;
use std::sync::Arc;
use std::{fmt, ptr::NonNull};

mod guts;
pub use guts::{Guard, IsPtr, Own, Ref, pin};

#[cfg(all(test, loom))]
mod loom_tests;
#[cfg(all(test, not(loom)))]
mod ui_tests;

impl<T: ?Sized> Ref<T> {
    pub fn with<O>(self, func: impl FnOnce(&T) -> O) -> Option<O> {
        self.get(&pin()).map(func)
    }

    pub fn map<R: ?Sized>(self, func: impl FnOnce(&T) -> &R) -> Ref<R> {
        self.map_with(func, &pin())
    }
}

impl<T: Send + 'static> Own<Box<T>> {
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

impl<T: ?Sized> IsPtr for Rc<T> {
    type T = T;

    fn into_raw_ptr(this: Self) -> NonNull<T> {
        NonNull::new(Rc::into_raw(this).cast_mut()).unwrap()
    }

    unsafe fn from_raw_ptr(ptr: NonNull<T>) -> Self {
        // SAFETY: same guarentees as the caller
        unsafe { Rc::from_raw(ptr.as_ptr()) }
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
