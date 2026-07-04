//! Thread-shared state connecting the UI / MIDI producers to the audio thread.
//!
//! The audio callback must never block, so it *tries* to lock and, on
//! contention, simply reuses the last patch it saw. Note events go through a
//! small queue that the callback drains at the top of each block.

use crate::preset::Patch;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// A MIDI-ish control message handed to the engine.
#[derive(Debug, Clone, Copy)]
pub enum MidiMsg {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    AllNotesOff,
}

/// State shared between the GUI/MIDI threads and the audio thread.
#[derive(Clone)]
pub struct SharedState {
    patch: Arc<Mutex<Patch>>,
    events: Arc<Mutex<VecDeque<MidiMsg>>>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new(Patch::default())
    }
}

impl SharedState {
    pub fn new(patch: Patch) -> Self {
        Self {
            patch: Arc::new(Mutex::new(patch)),
            events: Arc::new(Mutex::new(VecDeque::with_capacity(128))),
        }
    }

    /// Replace the current patch (called by the UI).
    pub fn set_patch(&self, patch: Patch) {
        if let Ok(mut guard) = self.patch.lock() {
            *guard = patch;
        }
    }

    /// Read the current patch. Returns `None` if the lock is momentarily busy.
    pub fn try_patch(&self) -> Option<Patch> {
        self.patch.try_lock().ok().map(|g| g.clone())
    }

    /// Blocking read of the current patch (for the UI thread).
    pub fn patch(&self) -> Patch {
        self.patch.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Queue a note event (from MIDI or the on-screen keyboard).
    pub fn push_event(&self, msg: MidiMsg) {
        if let Ok(mut q) = self.events.lock() {
            if q.len() < 1024 {
                q.push_back(msg);
            }
        }
    }

    /// Drain queued events into `out`. Non-blocking; keeps events if busy.
    pub fn drain_events(&self, out: &mut Vec<MidiMsg>) {
        if let Ok(mut q) = self.events.try_lock() {
            out.extend(q.drain(..));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_round_trip() {
        let s = SharedState::default();
        s.push_event(MidiMsg::NoteOn {
            note: 60,
            velocity: 100,
        });
        s.push_event(MidiMsg::NoteOff { note: 60 });
        let mut out = Vec::new();
        s.drain_events(&mut out);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn patch_set_get() {
        let s = SharedState::default();
        let p = Patch {
            overdrive: 0.9,
            ..Patch::default()
        };
        s.set_patch(p.clone());
        assert_eq!(s.patch().overdrive, 0.9);
    }
}
