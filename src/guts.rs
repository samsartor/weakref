use core::ops::Deref;
use core::ptr::NonNull;
use crossbeam_queue::SegQueue;
use std::mem::ManuallyDrop;

#[cfg(not(loom))]
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(loom)]
use loom::sync::atomic::{AtomicUsize, Ordering};

type CurrentGen = &'static AtomicUsize;
static RECYCLER: SegQueue<CurrentGen> = SegQueue::new();

#[allow(unused)]
pub(crate) fn empty_recycler() {
    while RECYCLER.pop().is_some() {}
}

pub use crossbeam_epoch::{Guard, pin};

pub trait IsPtr {
    type T: ?Sized;

    /// Converts to the raw pointer.
    fn into_raw_ptr(this: Self) -> NonNull<Self::T>;

    /// Converts from the raw pointer. This is used
    /// primarily to call the drop impl.
    ///
    /// # Safety
    /// The given pointer must have been recieved from [Self::into_raw_ptr].
    unsafe fn from_raw_ptr(ptr: NonNull<Self::T>) -> Self;
}

#[repr(transparent)]
pub struct Own<P: IsPtr + Send + 'static> {
    /// The weak reference.
    pub weak: Ref<P::T>,
}

impl<P: IsPtr + Send + 'static> Own<P> {
    pub fn new(ptr: P) -> Self {
        match RECYCLER.pop() {
            Some(ind) => Self::new_reuse(ind, ptr),
            None => Self::new_alloc(ptr),
        }
    }

    pub fn new_from<R: IsPtr + Send + 'static>(ptr: P, other: Own<R>) -> Self {
        Self::new_reuse(other.kill(&pin()).unwrap(), ptr)
    }

    fn new_reuse(current_gen: CurrentGen, ptr: P) -> Self {
        let pointer = Some(P::into_raw_ptr(ptr));
        let expected_gen = current_gen.load(Ordering::Acquire);
        Own {
            weak: Ref {
                current_gen,
                expected_gen,
                pointer,
            },
        }
    }

    fn new_alloc(ptr: P) -> Self {
        let pointer = Some(P::into_raw_ptr(ptr));
        let current_gen = Box::leak(Box::new(AtomicUsize::new(0)));
        let expected_gen = 0;
        Own {
            weak: Ref {
                current_gen,
                expected_gen,
                pointer,
            },
        }
    }

    fn kill(self, guard: &Guard) -> Option<CurrentGen> {
        let mut this = ManuallyDrop::new(self);
        // SAFETY: we move self and put it in manuallydrop, so it will not drop again
        unsafe { this.kill_mut(guard) }
    }

    /// # Safety
    /// Absolutely no use of `self` is permitted after calling this function,
    /// even to drop it.
    unsafe fn kill_mut(&mut self, guard: &Guard) -> Option<CurrentGen> {
        // Increment the generation counter with Release ordering so that no
        // [Ref::get] can access the pointer from now on. If a load has already
        // occured and the pointer is running around somewhere, the cleanup
        // will be defered until that thread is unpinned. Otherwise it may occur
        // immediately.
        let new_gen = self.weak.current_gen.fetch_add(1, Ordering::AcqRel) + 1;

        // Send the object to be dropped.
        let ptr = unsafe { P::from_raw_ptr(self.weak.pointer.unwrap()) };
        guard.defer(move || drop(ptr));

        // Recycle the generation counter, so long as it is possible to kill one more time.
        // Otherwise leak it forever, since it is completely unusable. This should
        // never happen in practice.
        if new_gen != usize::MAX {
            Some(self.weak.current_gen)
        } else {
            None
        }
    }
}

impl<P: IsPtr + Send + 'static> Drop for Own<P> {
    fn drop(&mut self) {
        let guard = pin();
        // SAFETY: we are in drop, so `self` will never be used again
        if let Some(ind) = unsafe { self.kill_mut(&guard) } {
            RECYCLER.push(ind);
        }
    }
}

impl<P: IsPtr + Send + 'static> Deref for Own<P> {
    type Target = P::T;

    fn deref(&self) -> &Self::Target {
        // Provide the reference.
        // SAFETY: this is always safe since `self` can not have been dropped
        unsafe { self.weak.pointer.unwrap().as_ref() }
    }
}

#[repr(C)]
pub struct Ref<T: ?Sized> {
    /// This Ref is only alive if the generation numbers match.
    current_gen: CurrentGen,
    expected_gen: usize,
    pointer: Option<NonNull<T>>,
}

impl<T: ?Sized> Clone for Ref<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Ref<T> {}

impl<T: ?Sized> Ref<T> {
    pub fn get(self, _guard: &Guard) -> Option<&T> {
        // As long as the generation number matches and the guard is active, the pointer will not have been freed.
        let current_gen = self.current_gen.load(Ordering::Acquire);
        if current_gen == self.expected_gen {
            Some(unsafe { self.pointer?.as_ref() })
        } else {
            None
        }
    }

    pub fn map_with<R: ?Sized>(&self, func: impl FnOnce(&T) -> &R, guard: &Guard) -> Ref<R> {
        Ref {
            current_gen: self.current_gen,
            expected_gen: self.expected_gen,
            pointer: match self.get(guard) {
                Some(value) => Some(NonNull::from_ref(func(value))),
                None => None,
            },
        }
    }
}
