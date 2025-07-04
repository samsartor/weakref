use core::ops::Deref;
use core::ptr::NonNull;
use crossbeam_queue::SegQueue;

#[cfg(not(loom))]
use core::sync::atomic::{AtomicUsize, Ordering};
#[cfg(loom)]
use loom::sync::atomic::{AtomicUsize, Ordering};

static RECYCLER: SegQueue<&'static Indirection> = SegQueue::new();

pub use crossbeam_epoch::{Guard, pin};

pub trait IsPtr {
    type T: ?Sized;

    /// Converts to the raw pointer.
    fn into_raw_ptr(this: Self) -> *mut Self::T;

    /// Converts from the raw pointer. This is used
    /// primarily to call the drop impl.
    ///
    /// # Safety
    /// The given pointer must have been recieved from [Self::into_raw_ptr].
    unsafe fn from_raw_ptr(ptr: *mut Self::T) -> Self;
}

#[repr(C)]
pub struct Indirection {
    pointer: AtomicUsize,
    current_gen: AtomicUsize,
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

    pub fn new_from<R: IsPtr + Send + 'static>(ptr: P, mut other: Own<R>) -> Self {
        Self::new_reuse(unsafe { other.kill_mut(&pin()) }.unwrap(), ptr)
    }

    fn new_reuse(ind: &'static Indirection, ptr: P) -> Self {
        let real = P::into_raw_ptr(ptr);
        ind.pointer.store(real.addr(), Ordering::Relaxed);
        let expected_gen = ind.current_gen.load(Ordering::Relaxed);
        let fake = real.with_addr((ind as *const Indirection).addr());
        Own {
            weak: Ref {
                indirection: fake,
                expected_gen,
            },
        }
    }

    fn new_alloc(ptr: P) -> Self {
        let real = P::into_raw_ptr(ptr);
        let ind = Box::leak(Box::new(Indirection {
            pointer: AtomicUsize::new(real.addr()),
            current_gen: AtomicUsize::new(0),
        }));
        let expected_gen = 0;
        let fake = real.with_addr((ind as *const Indirection).addr());
        Own {
            weak: Ref {
                indirection: fake,
                expected_gen,
            },
        }
    }

    /// # Safety
    /// Absolutely no use of `self` is permitted after calling this function.
    unsafe fn kill_mut(&mut self, guard: &Guard) -> Option<&'static Indirection> {
        let ind: &'static Indirection = unsafe { &*self.weak.indirection.cast() };

        // Increment the generation counter with Release ordering so that no [Ref::get] can
        // access the indirection from this moment onward. If a load has already occured and the
        // pointer is running around somewhere, the cleanup will be defered until that thread is
        // unpinned. Otherwise it may occur immediately.
        let new_gen = ind.current_gen.fetch_add(1, Ordering::Release);
        let address = ind.pointer.load(Ordering::Relaxed);

        // Get the real pointer from the address provided by the indirection.
        let real = self.weak.indirection.with_addr(address);

        // Send the object to be dropped.
        let ptr = unsafe { P::from_raw_ptr(real) };
        guard.defer(move || drop(ptr));

        // Recycle the indirection, so long as it is possible to kill one more time.
        // Otherwise leak it forever, since it is completely unusable. This should
        // never happen in practice.
        if new_gen != usize::MAX {
            Some(ind)
        } else {
            None
        }
    }
}

impl<P: IsPtr + Send + 'static> Drop for Own<P> {
    fn drop(&mut self) {
        let guard = pin();
        // SAFETY: we are in drop
        if let Some(ind) = unsafe { self.kill_mut(&guard) } {
            RECYCLER.push(ind);
        }
    }
}

impl<P: IsPtr + Send + 'static> Deref for Own<P> {
    type Target = P::T;

    fn deref(&self) -> &Self::Target {
        let ind: &'static Indirection = unsafe { &*self.weak.indirection.cast() };
        // Get the real pointer from the address provided by the indirection.
        let address = ind.pointer.load(Ordering::Relaxed);
        let real = self.weak.indirection.with_addr(address);

        // Provide the reference.
        unsafe { &*real }
    }
}

#[repr(C)]
pub struct Ref<T: ?Sized> {
    /// The pointer to the [Indirection]. We annotate it as `*mut T` so that
    /// the rust compiler includes the additional metadata for unsized T. Once
    /// std::ptr::Pointee is stable we'll correct the types.
    indirection: *mut T,
    /// This Ref is only alive if the generation numbers match.
    expected_gen: usize,
}

impl<T: ?Sized> Clone for Ref<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Ref<T> {}

impl<T: ?Sized> Ref<T> {
    #[inline]
    fn indirection(self) -> &'static Indirection {
        unsafe { &*self.indirection.cast() }
    }

    pub fn get(self, _guard: &Guard) -> Option<&T> {
        // As long as the generation number matches, the indirection will contain the same data as
        // when self was created. The guard will prevent any deletion screwing up the reference
        // we return. If a killed pointer was revived, we need to see any changes made to that pointer
        // before reviving so we use Acquire ordering.
        let current_gen = self.indirection().current_gen.load(Ordering::Acquire);
        if current_gen == self.expected_gen {
            let address = self.indirection().pointer.load(Ordering::Relaxed);

            // Get the real pointer from the address provided by the indirection.
            let real = self.indirection.with_addr(address);

            // Provide the reference.
            Some(unsafe { &*real })
        } else {
            None
        }
    }

    pub fn mapped(self) -> MappedRef<T> {
        let address = self.indirection().pointer.load(Ordering::Relaxed);
        let real = self.indirection.with_addr(address);
        let mapped = NonNull::new(real);
        MappedRef {
            indirection: self.indirection(),
            expected_gen: self.expected_gen,
            mapped,
        }
    }
}

#[repr(C)]
pub struct MappedRef<T: ?Sized> {
    indirection: &'static Indirection,
    expected_gen: usize,
    mapped: Option<NonNull<T>>,
}

impl<T: ?Sized> MappedRef<T> {
    pub fn get<'g>(&self, _guard: &'g Guard) -> Option<&'g T> {
        // As long as the generation number matches, the indirection will contain the same data as
        // when self was created. The guard will prevent any deletion screwing up the reference
        // we return. If a killed pointer was revived, we need to see any changes made to that pointer
        // before reviving so we use Acquire ordering.
        let current_gen = self.indirection.current_gen.load(Ordering::Acquire);
        if current_gen == self.expected_gen {
            // Provide the reference.
            Some(unsafe { self.mapped?.as_ref() })
        } else {
            None
        }
    }

    pub fn map_with<R: ?Sized>(&self, func: impl FnOnce(&T) -> &R, guard: &Guard) -> MappedRef<R> {
        MappedRef {
            indirection: self.indirection,
            expected_gen: self.expected_gen,
            mapped: self.get(guard).map(|value| NonNull::from_ref(func(value))),
        }
    }
}
