use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

mod guts;
pub use guts::{Guard, IsPtr, MappedRef, Own, Ref, pin};

impl<T: ?Sized> Ref<T> {
    pub fn with<O>(self, func: impl FnOnce(&T) -> O) -> Option<O> {
        self.get(&pin()).map(func)
    }

    pub fn map_with<R: ?Sized>(self, func: impl FnOnce(&T) -> &R, guard: &Guard) -> MappedRef<R> {
        self.mapped().map_with(func, guard)
    }

    pub fn map<R: ?Sized>(self, func: impl FnOnce(&T) -> &R) -> MappedRef<R> {
        self.mapped().map_with(func, &pin())
    }
}

impl<T: ?Sized> MappedRef<T> {
    pub fn with<O>(self, func: impl FnOnce(&T) -> O) -> Option<O> {
        self.get(&pin()).map(func)
    }

    pub fn map<R: ?Sized>(self, func: impl FnOnce(&T) -> &R) -> MappedRef<R> {
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
        // `.field` requires `T::Sized` and `field_with` is unstable
        //f.debug_tuple("Own").field(live).finish()
        write!(f, "Own({:?})", &**self)
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for Ref<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.get(&pin()) {
            Some(live) => {
                // `.field` requires `T::Sized` and `field_with` is unstable
                //f.debug_tuple("Ref::Live").field(live).finish()
                write!(f, "Ref::Live({live:?})")
            }
            None => f.debug_tuple("Ref::Dead").finish_non_exhaustive(),
        }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for MappedRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.get(&pin()) {
            Some(live) => {
                // `.field` requires `T::Sized` and `field_with` is unstable
                //f.debug_tuple("Ref::Live").field(live).finish()
                write!(f, "MappedRef::Live({live:?})")
            }
            None => f.debug_tuple("MappedRef::Dead").finish_non_exhaustive(),
        }
    }
}

impl<T: ?Sized> IsPtr for Box<T> {
    type T = T;

    fn into_raw_ptr(this: Self) -> *mut T {
        Box::into_raw(this)
    }

    unsafe fn from_raw_ptr(ptr: *mut T) -> Self {
        // SAFETY: same guarentees as the caller
        unsafe { Box::from_raw(ptr) }
    }
}

impl<T: ?Sized> IsPtr for Rc<T> {
    type T = T;

    fn into_raw_ptr(this: Self) -> *mut T {
        Rc::into_raw(this).cast_mut()
    }

    unsafe fn from_raw_ptr(ptr: *mut T) -> Self {
        // SAFETY: same guarentees as the caller
        unsafe { Rc::from_raw(ptr) }
    }
}

impl<T: ?Sized> IsPtr for Arc<T> {
    type T = T;

    fn into_raw_ptr(this: Self) -> *mut T {
        Arc::into_raw(this).cast_mut()
    }

    unsafe fn from_raw_ptr(ptr: *mut T) -> Self {
        // SAFETY: same guarentees as the caller
        unsafe { Arc::from_raw(ptr) }
    }
}

impl IsPtr for String {
    type T = str;

    fn into_raw_ptr(this: Self) -> *mut str {
        let b: Box<str> = this.into();
        IsPtr::into_raw_ptr(b)
    }

    unsafe fn from_raw_ptr(ptr: *mut str) -> Self {
        // SAFETY: same guarentees as the caller
        let b: Box<str> = unsafe { IsPtr::from_raw_ptr(ptr) };
        b.into()
    }
}

impl<T> IsPtr for Vec<T> {
    type T = [T];

    fn into_raw_ptr(this: Self) -> *mut [T] {
        let b: Box<[T]> = this.into();
        IsPtr::into_raw_ptr(b)
    }

    unsafe fn from_raw_ptr(ptr: *mut [T]) -> Self {
        // SAFETY: same guarentees as the caller
        let b: Box<[T]> = unsafe { IsPtr::from_raw_ptr(ptr) };
        b.into()
    }
}

impl IsPtr for () {
    type T = ();

    fn into_raw_ptr(_: Self) -> *mut Self::T {
        std::ptr::null_mut()
    }

    unsafe fn from_raw_ptr(_: *mut Self::T) -> Self {}
}
