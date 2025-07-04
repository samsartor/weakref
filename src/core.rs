use crossbeam_queue::SegQueue;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

static RECYCLER: SegQueue<&'static Indirection> = SegQueue::new();

pub use crossbeam_epoch::{Guard, pin};

pub trait IsPtr {
    type T: ?Sized;

    fn into_raw_ptr(this: Self) -> *mut Self::T;
    unsafe fn from_raw_ptr(ptr: *mut Self::T) -> Self;
}

pub struct Indirection {
    data: AtomicUsize,
    current_gen: AtomicU64,
}

#[repr(transparent)]
pub struct Own<P: IsPtr + Send + 'static> {
    pub weak: Ref<P::T>,
}

impl<P: IsPtr + Send + 'static> Own<P> {
    pub fn new(ptr: P) -> Self {
        let real = P::into_raw_ptr(ptr);
        let (ind, expected_gen) = match RECYCLER.pop() {
            Some(ind) => {
                ind.data.store(real.addr(), Ordering::Relaxed);
                (ind, ind.current_gen.load(Ordering::Relaxed))
            }
            None => {
                let ind = Box::leak(Box::new(Indirection {
                    data: AtomicUsize::new(real.addr()),
                    current_gen: AtomicU64::new(0),
                }));
                (&*ind, 0u64)
            }
        };
        let ind: *const Indirection = ind;
        let fake = real.with_addr(ind.addr());
        Own {
            weak: Ref { fake, expected_gen },
        }
    }
}

impl<P: IsPtr + Send + 'static> Drop for Own<P> {
    fn drop(&mut self) {
        let guard = pin();
        let ind: &'static Indirection = unsafe { &*self.weak.fake.cast() };

        // Increment the generation counter with Release ordering so that no [Ref::get] can
        // access the indirection from this moment onward. If a load has already occured and the
        // pointer is running around somewhere, the cleanup will be defered until that thread is
        // unpinned. Otherwise it may occur immediately.
        let new_gen = ind.current_gen.fetch_add(1, Ordering::Release);
        let address = ind.data.load(Ordering::Relaxed);

        // Get the real pointer from the address provided by the indirection.
        let real = self.weak.fake.with_addr(address);

        // Send the object to be dropped.
        let ptr = unsafe { P::from_raw_ptr(real) };
        guard.defer(move || drop(ptr));

        // Recycle the indirection, so long as it is possible to kill one more time.
        // Otherwise leak it forever, since it is completely unusable.
        if new_gen != u64::MAX {
            RECYCLER.push(ind);
        }
    }
}

impl<P: IsPtr + Send + 'static> Deref for Own<P> {
    type Target = P::T;

    fn deref(&self) -> &Self::Target {
        let ind: &'static Indirection = unsafe { &*self.weak.fake.cast() };
        // Get the real pointer from the address provided by the indirection.
        let address = ind.data.load(Ordering::Relaxed);
        let real = self.weak.fake.with_addr(address);

        // Provide the reference.
        unsafe { &*real }
    }
}

#[derive(Copy, Clone)]
pub struct Ref<T: ?Sized> {
    fake: *mut T,
    expected_gen: u64,
}

impl<T: ?Sized> Ref<T> {
    pub fn get<'g>(&self, _guard: &'g Guard) -> Option<&'g T> {
        let ind: &'static Indirection = unsafe { &*self.fake.cast() };
        // As long as the generation number matches, the indirection will contain the same data as
        // when self was created. The guard will prevent any deletion screwing up the reference
        // we return. If a killed pointer was revived, we need to see any changes made to that pointer
        // before reviving so we use Acquire ordering.
        let current_gen = ind.current_gen.load(Ordering::Acquire);
        if current_gen == self.expected_gen {
            let address = ind.data.load(Ordering::Relaxed);

            // Get the real pointer from the address provided by the indirection.
            let real = self.fake.with_addr(address);

            // Provide the reference.
            Some(unsafe { &*real })
        } else {
            None
        }
    }
}
