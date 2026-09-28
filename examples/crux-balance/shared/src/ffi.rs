//! The interface the shells call, generated for Swift and Kotlin by BoltFFI.
//!
//! Identical in shape to every other Crux core: events in, effects out, bytes
//! on both sides. Nothing Solana-specific lives here.

use {
    crate::Balance,
    crux_core::{
        Core,
        bridge::{Bridge, EffectId},
    },
};

pub struct CoreFfi {
    core: Bridge<Balance>,
}

impl Default for CoreFfi {
    fn default() -> Self {
        Self::new()
    }
}

#[boltffi::export]
impl CoreFfi {
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: Bridge::new(Core::new()),
        }
    }

    /// Send an event to the app and return the effects.
    ///
    /// # Panics
    /// If the event cannot be deserialized. In production, handle the error.
    #[must_use]
    pub fn update(&self, data: &[u8]) -> Vec<u8> {
        let mut effects = Vec::new();
        match self.core.update(data, &mut effects) {
            Ok(()) => effects,
            Err(err) => panic!("{err}"),
        }
    }

    /// Resolve an effect and return the effects that follow.
    ///
    /// # Panics
    /// If `data` cannot be deserialized or `id` is invalid.
    #[must_use]
    pub fn resolve(&self, id: u32, data: &[u8]) -> Vec<u8> {
        let mut effects = Vec::new();
        match self.core.resolve(EffectId(id), data, &mut effects) {
            Ok(()) => effects,
            Err(err) => panic!("{err}"),
        }
    }

    /// The current `ViewModel`.
    ///
    /// # Panics
    /// If the view cannot be serialized.
    #[must_use]
    pub fn view(&self) -> Vec<u8> {
        let mut view_model = Vec::new();
        match self.core.view(&mut view_model) {
            Ok(()) => view_model,
            Err(err) => panic!("{err}"),
        }
    }
}
