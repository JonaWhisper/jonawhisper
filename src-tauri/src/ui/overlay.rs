//! State an overlay shares with whichever backend draws it.
//!
//! Both follow one lifecycle — publish, hand a generation to a backend, retire
//! it on close — and writing that twice is how the pill and the strip drifted.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

pub(crate) struct Shared<S> {
    state: Mutex<Option<S>>,
    /// Bumped by every open(). A close/open pair inside one frame interval
    /// would otherwise leave the previous backend running against the new
    /// state, animating everything at twice the rate.
    generation: AtomicU32,
}

impl<S> Shared<S> {
    pub(crate) const fn new() -> Self {
        Self { state: Mutex::new(None), generation: AtomicU32::new(0) }
    }

    /// `None` when already open. The state lands before the backend has a
    /// window, so a setter racing right behind `open` finds it rather than
    /// warning into the void.
    pub(crate) fn open(&self, state: S) -> Option<u32> {
        let mut guard = self.state.lock().unwrap();
        if guard.is_some() {
            return None;
        }
        *guard = Some(state);
        Some(self.generation.fetch_add(1, Ordering::Relaxed) + 1)
    }

    /// `true` when this call is the one that closed it.
    pub(crate) fn close(&self) -> bool {
        self.state.lock().unwrap().take().is_some()
    }

    pub(crate) fn is_open(&self) -> bool {
        self.state.lock().unwrap().is_some()
    }

    pub(crate) fn read<T>(&self, f: impl FnOnce(&S) -> T) -> Option<T> {
        self.state.lock().unwrap().as_ref().map(f)
    }

    pub(crate) fn write<T>(&self, f: impl FnOnce(&mut S) -> T) -> Option<T> {
        self.state.lock().unwrap().as_mut().map(f)
    }

    /// Mutate on behalf of a backend. `None` once the overlay is closed **or**
    /// superseded, which is how a backend learns to tear its window down.
    pub(crate) fn update<T>(&self, generation: u32, f: impl FnOnce(&mut S) -> T) -> Option<T> {
        if self.generation.load(Ordering::Relaxed) != generation {
            return None;
        }
        self.write(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_twice_yields_no_second_generation() {
        let shared: Shared<u8> = Shared::new();
        assert_eq!(shared.open(1), Some(1));
        assert_eq!(shared.open(2), None, "deja ouverte");
        assert_eq!(shared.read(|v| *v), Some(1), "l'etat d'origine survit");
    }

    #[test]
    fn closing_reports_who_did_it() {
        let shared: Shared<u8> = Shared::new();
        shared.open(1);
        assert!(shared.close());
        assert!(!shared.close(), "la seconde fermeture ne ferme rien");
        assert!(!shared.is_open());
    }

    #[test]
    fn a_superseded_backend_stops_updating() {
        let shared: Shared<u8> = Shared::new();
        let first = shared.open(1).unwrap();
        shared.close();
        let second = shared.open(2).unwrap();

        assert_ne!(first, second);
        assert_eq!(shared.update(first, |v| *v), None, "l'ancien backend est retire");
        assert_eq!(shared.update(second, |v| *v), Some(2));
    }

    #[test]
    fn accessors_report_nothing_once_closed() {
        let shared: Shared<u8> = Shared::new();
        let generation = shared.open(1).unwrap();
        shared.close();
        assert_eq!(shared.read(|v| *v), None);
        assert_eq!(shared.write(|v| *v), None);
        assert_eq!(shared.update(generation, |v| *v), None);
    }
}
