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

use std::collections::HashMap;

use super::Config;

impl Config {
    pub const fn default_value_keep_std() -> bool {
        true
    }

    pub const fn default_value_scene_game_list() -> bool {
        true
    }

    pub const fn default_value_feas_enable() -> bool {
        true
    }

    pub const fn default_value_feas_jank_thres_us() -> i32 {
        700
    }

    pub const fn default_value_feas_rescue_perf() -> bool {
        true
    }

    pub const fn default_value_feas_rescue_step_us() -> i32 {
        750
    }

    pub const fn default_value_feas_predict_thres_us() -> i32 {
        380
    }

    pub const fn default_value_feas_predict_perf() -> bool {
        false
    }

    pub const fn default_value_feas_predict_step_us() -> i32 {
        750
    }

    pub const fn default_value_feas_keepdown_thres_us() -> i32 {
        -50
    }

    pub const fn default_value_feas_keepdown_cooldown() -> i32 {
        3
    }

    pub const fn default_value_feas_nor_keep() -> i32 {
        12
    }

    pub const fn default_value_feas_jank_keep() -> i32 {
        25
    }

    pub const fn default_value_feas_cons_no_jank() -> i32 {
        10
    }

    pub const fn default_value_feas_release_floor_ms() -> u32 {
        2333
    }

    pub const fn default_value_feas_floor_freq() -> isize {
        384_000
    }

    pub const fn default_value_feas_release_floor_freq() -> isize {
        384_000
    }

    pub const fn default_value_feas_max_level() -> i32 {
        0
    }

    pub const fn default_value_feas_step() -> usize {
        1
    }

    pub const fn default_value_feas_max_frame_us() -> u64 {
        100_000
    }

    pub fn default_value_feas_hold_timeout_ms() -> HashMap<u32, u32> {
        HashMap::from([
            (144, 9100),
            (120, 13_000),
            (90, 16_600),
            (60, 25_000),
            (49, 27_000),
            (30, 44_000),
        ])
    }
}

impl super::ModeConfig {
    pub const fn default_value_feas_force_boost() -> bool {
        true
    }
}
