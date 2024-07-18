use std::cell::UnsafeCell;
use std::hint::spin_loop;
use std::marker::Sync;
use std::sync::atomic::{AtomicBool, Ordering};

/// Heap-less lock implementation. The allocator needs a lock that doesn't interact with it (a stack based lock)
pub struct Spinlock<T> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for Spinlock<T> {}

impl<T> Spinlock<T> {
    pub const fn new(data: T) -> Self {
        Spinlock {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinlockGuard<T> {
        // Acquire ordering guarantess that all read and writes that happen after the lock acquisition
        // are NOT moved before the actual acquisition. It is an 'acquire' fence
        while self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire)
            .unwrap_or(false)
        {
            spin_loop(); // Suggest yielding to the processor
        }
        SpinlockGuard { spinlock: self }
    }
}
/// Associates lifetime of the guard to the lifetime of the lock being held
pub struct SpinlockGuard<'a, T> {
    spinlock: &'a Spinlock<T>,
}

impl<'a, T> Drop for SpinlockGuard<'a, T> {
    fn drop(&mut self) {
        self.spinlock.lock.store(false, Ordering::Release);
    }
}

impl<'a, T> std::ops::Deref for SpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.spinlock.data.get() }
    }
}

impl<'a, T> std::ops::DerefMut for SpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.spinlock.data.get() }
    }
}
