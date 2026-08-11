// Copyright 2024-2025, shadow3aaa
//
// This file is part of fas-rs.
//
// fas-rs is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.
//
// fas-rs is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with fas-rs. If not, see <https://www.gnu.org/licenses/>.

mod default;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use toml::Table;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigData {
    pub config: Config,
    pub game_list: Table,
    #[serde(skip)]
    pub scene_game_list: HashSet<String>,
    pub powersave: ModeConfig,
    pub balance: ModeConfig,
    pub performance: ModeConfig,
    pub fast: ModeConfig,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Config {
    #[serde(default = "Config::default_value_keep_std")]
    pub keep_std: bool,
    #[serde(default = "Config::default_value_scene_game_list")]
    pub scene_game_list: bool,
    #[serde(default = "Config::default_value_mode")]
    pub default_mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Powersave,
    Balance,
    Performance,
    Fast,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Balance
    }
}

impl<'de> Deserialize<'de> for Mode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "powersave" => Ok(Mode::Powersave),
            "balance" => Ok(Mode::Balance),
            "performance" => Ok(Mode::Performance),
            "fast" => Ok(Mode::Fast),
            _ => Err(serde::de::Error::custom(format!("Invalid mode: {s}"))),
        }
    }
}

impl Serialize for Mode {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Mode::Powersave => serializer.serialize_str("powersave"),
            Mode::Balance => serializer.serialize_str("balance"),
            Mode::Performance => serializer.serialize_str("performance"),
            Mode::Fast => serializer.serialize_str("fast"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum TemperatureThreshold {
    #[serde(rename = "disabled")]
    Disabled,
    #[serde(untagged)]
    Temp(u64),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MarginFps {
    #[serde(untagged)]
    BaseOnly(MarginFpsValue),
    #[serde(untagged)]
    Advanced {
        base: MarginFpsValue,
        #[serde(flatten)]
        overrides: HashMap<String, MarginFpsValue>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum MarginFpsValue {
    #[serde(untagged)]
    Float(f64),
    #[serde(untagged)]
    Int(u64),
}

impl From<MarginFpsValue> for f64 {
    fn from(value: MarginFpsValue) -> Self {
        match value {
            MarginFpsValue::Float(f) => f,
            MarginFpsValue::Int(i) => i as Self,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename = "map")]
pub struct SceneAppList {
    #[serde(rename = "boolean")]
    pub apps: Vec<SceneApp>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SceneApp {
    #[serde(rename = "@name")]
    pub pkg: String,
    #[serde(rename = "@value")]
    pub is_game: bool,
}
