use core::ops::Deref;
use core::ptr::NonNull;
use crossbeam_epoch::{Guard, pin};
use crossbeam_queue::SegQueue;
use std::{cell::RefCell, mem::ManuallyDrop};

#[cfg(not(loom))]
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(loom)]
use loom::sync::atomic::{AtomicUsize, Ordering};

type GenerationCounter = &'static AtomicUsize;
static GLOBAL_RECYCLER: SegQueue<[GenerationCounter; BLOCK_SIZE]> = SegQueue::new();
thread_local! {
    static LOCAL_RECYCLER: RefCell<Vec<GenerationCounter>> = RefCell::new(Vec::with_capacity(BLOCK_SIZE*2));
}

#[cfg(loom)]
const BLOCK_SIZE: usize = 16;
#[cfg(not(loom))]
const BLOCK_SIZE: usize = 256;

pub(crate) fn new_generation_counter() -> GenerationCounter {
    LOCAL_RECYCLER.with_borrow_mut(|local_recycler| {
        if let Some(counter) = local_recycler.pop() {
            return counter;
        }
        if let Some(block) = GLOBAL_RECYCLER.pop() {
            let [next, rest @ ..] = block;
            local_recycler.extend(rest);
            return next;
        }
        let block: [AtomicUsize; BLOCK_SIZE] = std::array::from_fn(|_| AtomicUsize::new(0));
        let block = Box::leak(Box::new(block));
        local_recycler.extend(block.iter());
        local_recycler.pop().unwrap()
    })
}

pub(crate) fn recycle_generation_counter(counter: GenerationCounter) {
    LOCAL_RECYCLER.with_borrow_mut(|local_recycler| {
        if local_recycler.len() == local_recycler.capacity() {
            let block: [GenerationCounter; BLOCK_SIZE] =
                std::array::from_fn(|_| local_recycler.pop().unwrap());
            GLOBAL_RECYCLER.push(block);
        }
        local_recycler.push(counter);
    })
}

#[allow(unused)]
pub(crate) fn empty_recycler() {
    LOCAL_RECYCLER.with_borrow_mut(|r| r.clear());
    while GLOBAL_RECYCLER.pop().is_some() {}
}

#[cfg(test)]
pub(crate) fn local_recycler_len() -> usize {
    LOCAL_RECYCLER.with_borrow(|r| r.len())
}

#[cfg(test)]
pub(crate) fn global_recycler_len() -> usize {
    GLOBAL_RECYCLER.len()
}

/// Implemented for any owning pointer.
///
/// # Safety
/// When accepting an unknown `impl IsPtr`, be aware of the various guarantees
/// expected by all implementors. In particular, it may not be safe to mutate
/// the pointer since `Pin<T>: IsPtr`.
pub trait IsPtr {
    type T: ?Sized;

    /// Converts to the raw pointer.
    fn into_raw_ptr(this: Self) -> NonNull<Self::T>;

    /// Converts from the raw pointer. This is used mainly to call drop.
    ///
    /// # Safety
    /// The given pointer must have been returned by [Self::into_raw_ptr].
    unsafe fn from_raw_ptr(ptr: NonNull<Self::T>) -> Self;
}

/// Unique owner for a value, which will inform references when dropped.
#[repr(transparent)]
pub struct Own<P: IsPtr + Send + 'static> {
    /// The weak reference. _SAFETY: Do not mutate._
    ///
    /// It would be nice to make this public, but there are soundness
    /// issues with allowing users to reassign it. Instead we limit access
    /// to the `refer` method and macro.
    ///
    /// ```ignore
    /// let a = Own::new_box(42);
    /// let mut b = Own::new_box(43);
    /// b._weak = a._weak;
    /// std::thread::spawn(mut || { drop(b); });
    /// std::thread::spawn(mut || { *a; });
    /// ```
    #[doc(hidden)]
    pub _weak: Ref<P::T>,
}

impl<P: IsPtr + Send + 'static> Own<P> {
    /// Wrap the given pointer so that it can inform weak references when dropped.
    pub fn new(ptr: P) -> Self {
        Self::new_reuse(new_generation_counter(), ptr)
    }

    /// Like [Own::new], but cheaper if an existing owned needs to be dropped.
    /// The generation counter can be incremented and reused without checking the global pool.
    pub fn new_from<R: IsPtr + Send + 'static>(ptr: P, other: Own<R>) -> Self {
        Self::new_reuse(other.kill(&pin()).unwrap(), ptr)
    }

    /// Provides the weak pointer.
    pub fn refer(&self) -> Ref<P::T> {
        self._weak
    }

    fn new_reuse(current_gen: GenerationCounter, ptr: P) -> Self {
        let pointer = Some(P::into_raw_ptr(ptr));
        let expected_gen = current_gen.load(Ordering::Acquire);
        Own {
            _weak: Ref {
                current_gen,
                expected_gen,
                pointer,
            },
        }
    }

    fn kill(self, guard: &Guard) -> Option<GenerationCounter> {
        let mut this = ManuallyDrop::new(self);
        // SAFETY: self is moved into ManuallyDrop, preventing double-drop
        unsafe { this.kill_mut(guard) }
    }

    /// # Safety
    /// Absolutely no use of `self` is permitted after calling this function,
    /// even to drop it.
    unsafe fn kill_mut(&mut self, guard: &Guard) -> Option<GenerationCounter> {
        // Increment the generation counter with Release ordering so that no
        // [Ref::get] can access the pointer from now on. If a load has already
        // occurred and the pointer is running around somewhere, the cleanup
        // will be deferred until that thread is unpinned. Otherwise it may occur
        // immediately.
        let new_gen = self._weak.expected_gen + 1;
        if self
            ._weak
            .current_gen
            .compare_exchange(
                self._weak.expected_gen,
                new_gen,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_err()
        {
            panic!("Tried to drop a dead reference. Did you mutate Own._weak?");
        }

        // Send the object to be dropped.
        let ptr = unsafe { P::from_raw_ptr(self._weak.pointer.take().unwrap()) };
        guard.defer(move || drop(ptr));

        // Recycle the generation counter, so long as it is possible to kill one more time.
        // Otherwise leak it forever, since it is completely unusable. This should
        // never happen in practice.
        if new_gen != usize::MAX {
            Some(self._weak.current_gen)
        } else {
            None
        }
    }
}

impl<P: IsPtr + Send + 'static> Drop for Own<P> {
    fn drop(&mut self) {
        let guard = pin();
        // SAFETY: Called from Drop::drop, so self will never be used again
        if let Some(counter) = unsafe { self.kill_mut(&guard) } {
            recycle_generation_counter(counter);
        }
    }
}

impl<P: IsPtr + Send + 'static> Deref for Own<P> {
    type Target = P::T;

    fn deref(&self) -> &Self::Target {
        // Provide the reference.
        // SAFETY: Owner is alive, so pointer is valid and generation matches
        unsafe { self._weak.pointer.unwrap().as_ref() }
    }
}

unsafe impl<P: IsPtr + Send> Send for Own<P> where P::T: Sync {}
unsafe impl<P: IsPtr + Send> Sync for Own<P> where P::T: Sync {}

/// Weak reference for a value which checks liveness at runtime.
#[repr(C)]
pub struct Ref<T: ?Sized> {
    /// This Ref is only alive if the generation numbers match.
    current_gen: GenerationCounter,
    expected_gen: usize,
    pointer: Option<NonNull<T>>,
}

unsafe impl<T: Sync + ?Sized> Send for Ref<T> {}
unsafe impl<T: Sync + ?Sized> Sync for Ref<T> {}

impl<T: ?Sized> Clone for Ref<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Ref<T> {}

impl<T: ?Sized> Ref<T> {
    /// Check if the original owner has been dropped. If it is alive, return the reference.
    ///
    /// __The [Ref::get] method is the point of the weakref crate__
    ///
    /// ```
    ///# use weakref::{Own, pin};
    /// let data = Own::new_box(42);
    /// let weak = data.refer();
    /// assert_eq!(weak.get(&pin()), Some(&42));
    /// drop(data);
    /// assert_eq!(weak.get(&pin()), None);
    /// ```
    ///
    /// Notice that the returned reference only borrows from [Guard]. Until the thread is unpinned,
    /// the generation counter does not need to be re-checked.
    pub fn get(self, _guard: &Guard) -> Option<&T> {
        // Acquire ordering ensures we see the latest generation - if it matches,
        // the epoch guard prevents the pointer from being freed
        let current_gen = self.current_gen.load(Ordering::Acquire);
        if current_gen == self.expected_gen {
            Some(unsafe { self.pointer?.as_ref() })
        } else {
            None
        }
    }

    /// [Pin](pin) the current thread and check if the owner has been dropped. If it is alive, call `func` and return the output.
    pub fn inspect<O>(self, func: impl FnOnce(&T) -> O) -> Option<O> {
        self.get(&pin()).map(func)
    }

    /// Produces a new weak reference tied to self, which points to something reachable through the original pointer.
    /// ```
    ///# use weakref::{Own, Ref, pin};
    /// let list = Own::new(vec![1, 2, 3]);
    /// let elem: Ref<i32> = list.refer().map(|x| &x[2]);
    /// assert_eq!(elem.get(&pin()), Some(&3));
    /// drop(list);
    /// assert_eq!(elem.get(&pin()), None);
    /// ```
    pub fn map<R: ?Sized>(self, func: impl FnOnce(&T) -> &R) -> Ref<R> {
        self.map_with(func, &pin())
    }

    /// Like [Ref::map], but cheaper if a thread guard is already available.
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

    /// Like [Self::map], but with the ability to to produce [Ref::null].
    /// ```
    ///# use weakref::{Own, Ref, pin};
    /// let list = Own::new(vec![1, 2, 3]);
    /// let elem: Ref<i32> = list.refer().filter_map(|x| x.get(100));
    /// assert_eq!(elem.get(&pin()), None);
    /// ```
    pub fn filter_map<R: ?Sized>(self, func: impl FnOnce(&T) -> Option<&R>) -> Ref<R> {
        self.filter_map_with(func, &pin())
    }

    /// Like [Ref::map], but cheaper if a thread guard is already available.
    pub fn filter_map_with<R: ?Sized>(
        &self,
        func: impl FnOnce(&T) -> Option<&R>,
        guard: &Guard,
    ) -> Ref<R> {
        Ref {
            current_gen: self.current_gen,
            expected_gen: self.expected_gen,
            pointer: match self.get(guard) {
                Some(value) => func(value).map(NonNull::from_ref),
                None => None,
            },
        }
    }

    /// Checks whether the owner has been dopped.
    ///
    /// Be aware there are no ordering guarentees on this function. If true
    /// is returned then the reference was alive at _some point_, but may
    /// have been killed between the counter being checked and this function
    /// returning.
    pub fn is_alive(&self) -> bool {
        let current_gen = self.current_gen.load(Ordering::Relaxed);
        current_gen == self.expected_gen && self.pointer.is_some()
    }

    /// Returns true if this reference is exactly [Ref::null].
    ///
    /// Note [Ref::map] and [Ref::filter_map] will return null when called on a dead
    /// reference or when the filter returns None. But a reference is guarenteed
    /// to never become null if considered alive at any point previously (only
    /// loose that liveness).
    pub fn is_null(&self) -> bool {
        self.pointer.is_none()
    }

    /// Returns a fake reference where [Ref::get] is always None, as if the owner was dropped.
    /// ```
    ///# use weakref::{Ref, pin};
    /// let null = Ref::<i32>::null();
    /// assert_eq!(null.get(&pin()), None);
    /// ```
    #[cfg(not(loom))]
    pub const fn null() -> Self {
        static STATIC_GEN: AtomicUsize = AtomicUsize::new(usize::MAX);
        Ref {
            current_gen: &STATIC_GEN,
            expected_gen: 0,
            pointer: None,
        }
    }
}
