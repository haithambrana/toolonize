//! Lock helper. A panic while a lock is held poisons it; with
//! `lock().unwrap()` every later command touching that state would then
//! panic too. The data behind our locks stays consistent between statements
//! (mutations are applied only after a successful save), so recovering the
//! guard keeps the rest of the app working.

use std::sync::{Mutex, MutexGuard, PoisonError};

pub trait Locked<T> {
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> Locked<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn a_poisoned_lock_stays_usable() {
        let m = Arc::new(Mutex::new(1));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("boom");
        })
        .join();
        assert!(m.is_poisoned());
        *m.locked() += 1;
        assert_eq!(*m.locked(), 2);
    }
}
