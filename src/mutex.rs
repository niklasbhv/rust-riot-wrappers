use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
// For correctness considerations, all uses of UnsafeCell can be ignored here; the only reason why
// an UnsafeCell is used is to indicate to the linker that a static mutex still needs to be
// allocated in .data and not in .text. (In other words: This is what allows transmuting the & to
// the inner data into a &mut).
use core::cell::UnsafeCell;

/// A mutual exclusion primitive useful for protecting shared data
///
/// Unlike the std::sync::Mutex, this has no concept of poisoning, so waiting for mutexes in
/// paniced (and thus locked) threads will lock the accessing thread as well. This is because RIOT
/// threds don't unwind Rust code. As a consequence, the mutex interface is different from the
/// standard library's.
///
/// Several methods (into_inner, get_mut) are not implemented until they're actually needed.
pub struct Mutex<T> {
    mutex: UnsafeCell<riot_sys::mutex_t>,
    data: UnsafeCell<T>,
}

impl<T> Mutex<T> {
    /// Create a new mutex
    pub const fn new(t: T) -> Mutex<T> {
        let mut new = MaybeUninit::uninit();
        unsafe {
            riot_sys::mutex_init(new.as_mut_ptr() as *mut _ /* INLINE CAST */);
        };
        // unsafe: initialized as per C API description
        // FIXME transmute should be an assume_init but that's not const yet
        let new = unsafe { core::mem::transmute(new) };
        Mutex {
            data: UnsafeCell::new(t),
            mutex: UnsafeCell::new(new),
        }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        unsafe { riot_sys::mutex_lock(self.mutex.get() as *mut _ /* INLINE CAST */) };
        MutexGuard { mutex: &self }
    }

    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        match unsafe { riot_sys::mutex_trylock(self.mutex.get() as *mut _ /* INLINE CAST */) } {
            1 => Some(MutexGuard { mutex: &self }),
            _ => None,
        }
    }
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

pub struct MutexGuard<'a, T> {
    mutex: &'a Mutex<T>,
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        unsafe { riot_sys::mutex_unlock(self.mutex.mutex.get()) }
    }
}

impl<'a, T> MutexGuard<'a, T> {
    /// Put the current thread to sleep right after unlocking the mutex. This is equivalent to
    /// calling mutex_unlock_and_sleep in RIOT.
    pub fn unlock_and_sleep(self) {
        let m = &self.mutex.mutex;
        ::core::mem::forget(self);
        unsafe { riot_sys::mutex_unlock_and_sleep(m.get()) };
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.mutex.data.get() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *(self.mutex.data.get()) }
    }
}
