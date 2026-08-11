use super::Node;

// Mode is defined in crate::framework::config::data::Mode.
// This module exists only for backwards compatibility with existing code that
// imports `node::Mode`. Re-export it here so nothing breaks.
pub use crate::framework::config::Mode;

impl Node {
    /// Get the current power mode.
    /// Since fas-rs is now independent of external schedulers, this always returns Balance.
    /// The actual mode is determined by Config::default_mode().
    pub fn get_mode(&mut self) -> crate::framework::error::Result<Mode> {
        Ok(Mode::Balance)
    }
}
