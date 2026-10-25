use std::{ops, sync};

/// A synchronous mutual exclusion primitive useful for protecting shared data.
#[derive(Default, Debug)]
pub struct Mutex<T>(sync::Mutex<T>);

impl<T> Mutex<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    pub fn new(value: T) -> Self {
        Self(sync::Mutex::new(value))
    }

    /// Acquires a mutex, blocking the current thread until it is able to do so.
    pub fn lock(&self) -> MutexGuard<'_, T> {
        let guard = self.0.lock().unwrap();

        MutexGuard(guard)
    }

    /// Consumes this mutex, returning the underlying data.
    pub fn into_inner(self) -> T {
        self.0.into_inner().unwrap()
    }
}

/// An RAII implementation of a "scoped lock" of a mutex. When this structure is
/// dropped (falls out of scope), the lock will be unlocked.
pub struct MutexGuard<'a, T>(sync::MutexGuard<'a, T>);

impl<T> ops::Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> ops::DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A synchronous reader-writer lock.
#[derive(Default, Debug)]
pub struct RwLock<T>(sync::RwLock<T>);

impl<T> RwLock<T> {
    /// Creates a new mutex in an unlocked state ready for use.
    pub fn new(value: T) -> Self {
        Self(sync::RwLock::new(value))
    }

    /// Locks this rwlock with shared read access, blocking the current thread
    /// until it can be acquired.
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        let guard = self.0.read().unwrap();

        RwLockReadGuard(guard)
    }

    /// Locks this rwlock with exclusive write access, blocking the current
    /// thread until it can be acquired.
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        let guard = self.0.write().unwrap();

        RwLockWriteGuard(guard)
    }
}

/// RAII structure used to release the shared read access of a lock when
/// dropped.
pub struct RwLockReadGuard<'a, T>(sync::RwLockReadGuard<'a, T>);

impl<T> ops::Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// RAII structure used to release the exclusive write access of a lock when
/// dropped.
pub struct RwLockWriteGuard<'a, T>(sync::RwLockWriteGuard<'a, T>);

impl<T> ops::Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> ops::DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
