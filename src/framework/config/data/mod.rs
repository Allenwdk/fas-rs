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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "Config::default_value_keep_std")]
    pub keep_std: bool,
    #[serde(default = "Config::default_value_scene_game_list")]
    pub scene_game_list: bool,
    #[serde(default = "Config::default_value_feas_enable")]
    pub feas_enable: bool,
    #[serde(default = "Config::default_value_feas_jank_thres_us")]
    pub feas_jank_thres_us: i32,
    #[serde(default = "Config::default_value_feas_rescue_perf")]
    pub feas_rescue_perf: bool,
    #[serde(default = "Config::default_value_feas_rescue_step_us")]
    pub feas_rescue_step_us: i32,
    #[serde(default = "Config::default_value_feas_predict_thres_us")]
    pub feas_predict_thres_us: i32,
    #[serde(default = "Config::default_value_feas_predict_perf")]
    pub feas_predict_perf: bool,
    #[serde(default = "Config::default_value_feas_predict_step_us")]
    pub feas_predict_step_us: i32,
    #[serde(default = "Config::default_value_feas_keepdown_thres_us")]
    pub feas_keepdown_thres_us: i32,
    #[serde(default = "Config::default_value_feas_keepdown_cooldown")]
    pub feas_keepdown_cooldown: i32,
    #[serde(default = "Config::default_value_feas_nor_keep")]
    pub feas_nor_keep: i32,
    #[serde(default = "Config::default_value_feas_jank_keep")]
    pub feas_jank_keep: i32,
    #[serde(default = "Config::default_value_feas_cons_no_jank")]
    pub feas_cons_no_jank: i32,
    #[serde(default = "Config::default_value_feas_hold_timeout_ms")]
    pub feas_hold_timeout_ms: HashMap<u32, u32>,
    #[serde(default = "Config::default_value_feas_release_floor_ms")]
    pub feas_release_floor_ms: u32,
    #[serde(default = "Config::default_value_feas_floor_freq")]
    pub feas_floor_freq: isize,
    #[serde(default = "Config::default_value_feas_release_floor_freq")]
    pub feas_release_floor_freq: isize,
    #[serde(default = "Config::default_value_feas_max_level")]
    pub feas_max_level: i32,
    #[serde(default = "Config::default_value_feas_step")]
    pub feas_step: usize,
    #[serde(default = "Config::default_value_feas_max_frame_us")]
    pub feas_max_frame_us: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModeConfig {
    pub margin_fps: MarginFps,
    pub core_temp_thresh: TemperatureThreshold,
    #[serde(default = "ModeConfig::default_value_feas_force_boost")]
    pub feas_force_boost: bool,
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
