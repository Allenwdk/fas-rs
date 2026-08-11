use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use super::Node;
use crate::framework::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Powersave,
    Balance,
    Performance,
    Fast,
}

impl FromStr for Mode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s {
            "powersave" => Self::Powersave,
            "balance" => Self::Balance,
            "performance" => Self::Performance,
            "fast" => Self::Fast,
            _ => return Err(Error::ParseNode),
        })
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let mode = match self {
            Self::Powersave => "powersave",
            Self::Balance => "balance",
            Self::Performance => "performance",
            Self::Fast => "fast",
        };

        write!(f, "{mode}")
    }
}

impl Node {
    /// Get the current power mode.
    /// Since fas-rs is now independent of external schedulers, this always returns Balance.
    /// The actual mode is determined by Config::default_mode().
    pub fn get_mode(&mut self) -> Result<Mode> {
        // Mode is now config-driven, return default (Balance) as fallback
        Ok(Mode::Balance)
    }
}
