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

use super::{ConfigData, ModeConfig};
/// FEAS 策略参数（从配置构造，纯数据）。
///
/// 对应 xiaomifeas `perfmgr_policy.c` 的 module_param 组合：
/// - `jank_thres` = scaling_r_thres（jank 线 = 1e6 + jank_thres×1000）
/// - `rescue_perf` = r_perf，`rescue_step` = r_step
/// - `predict_thres` = scaling_a，`predict_perf` = p_perf，`predict_step` = p_step
/// - `keepdown_thres` = scaling_b，`keepdown_cooldown` = scaling_c
/// - `nor_keep`/`jank_keep` = nor_f_keep / j_f_k_count，`cons_no_jank` = cons_no_j_cnt
/// - `hold_timeout_ms` = timeout_144/120/90/60/49/30，`release_floor_ms` = timeout_left
/// - `floor_freq`/`release_floor_freq` = f_minfreq / f_left_minfreq
#[derive(Debug, Clone)]
pub struct FeasParams {
    /// jank 线偏移（×1000μs），默认 700 → 1.7e6
    pub jank_thres: i32,
    /// 救援性能模式：true=5 级救援，false=2 级
    pub rescue_perf: bool,
    /// 救援步长（×1000μs），默认 750
    pub rescue_step: i32,
    /// 预测提频线偏移（×1000μs），默认 380 → 5.38e6
    pub predict_thres: i32,
    /// 预测性能模式：true=3 级，false=2 级
    pub predict_perf: bool,
    /// 预测步长（×1000μs），默认 750
    pub predict_step: i32,
    /// 降频保持线偏移（×1000μs），默认 -50 → 5.05e6
    pub keepdown_thres: i32,
    /// 降频冷却（秒数系数）：`Δt(μs) × fps > cooldown×1e6`
    pub keepdown_cooldown: i32,
    /// 正常保持帧数
    pub nor_keep: i32,
    /// jank 后额外保持帧数
    pub jank_keep: i32,
    /// 连续无卡顿帧数阈值
    pub cons_no_jank: i32,
    /// 各目标 fps 对应的提频保持时长（ms）
    pub hold_timeout_ms: HashMap<u32, u32>,
    /// 释放阶段间间隔（ms）
    pub release_floor_ms: u32,
    /// 最大档位（0=自动取设备 freqs.len()-1）
    pub max_level: i32,
    /// 每级跨越的频点数
    pub step: usize,
    /// Limit 阶段地板频率（kHz；0=最低频，由写频侧解析）
    pub floor_freq: isize,
    /// Floor 阶段地板频率（kHz；0=最低频）
    pub release_floor_freq: isize,
    /// Limit 阶段是否锁死 min=max（force），否则 ceiling-only
    pub force_boost: bool,
    /// 兜底帧时长上限（μs），防巨型帧污染窗口
    pub max_frame_us: u64,
}

impl FeasParams {
    /// 默认参数（等价于 xiaomifeas 默认 module_param 组合）。
    #[allow(dead_code)]
    pub fn default_params() -> Self {
        Self {
            jank_thres: 700,
            rescue_perf: true,
            rescue_step: 750,
            predict_thres: 380,
            predict_perf: false,
            predict_step: 750,
            keepdown_thres: -50,
            keepdown_cooldown: 3,
            nor_keep: 12,
            jank_keep: 25,
            cons_no_jank: 10,
            hold_timeout_ms: HashMap::from([
                (144, 9100),
                (120, 13_000),
                (90, 16_600),
                (60, 25_000),
                (49, 27_000),
                (30, 44_000),
            ]),
            release_floor_ms: 2333,
            max_level: 0,
            step: 1,
            floor_freq: 384_000,
            release_floor_freq: 384_000,
            force_boost: true,
            max_frame_us: 100_000,
        }
    }

    /// 从配置构造（mode 用于取 per-mode 的 `feas_force_boost`）。
    pub fn from_mode(mode: &ModeConfig, c: &ConfigData) -> Self {
        Self {
            jank_thres: c.config.feas_jank_thres_us,
            rescue_perf: c.config.feas_rescue_perf,
            rescue_step: c.config.feas_rescue_step_us,
            predict_thres: c.config.feas_predict_thres_us,
            predict_perf: c.config.feas_predict_perf,
            predict_step: c.config.feas_predict_step_us,
            keepdown_thres: c.config.feas_keepdown_thres_us,
            keepdown_cooldown: c.config.feas_keepdown_cooldown,
            nor_keep: c.config.feas_nor_keep,
            jank_keep: c.config.feas_jank_keep,
            cons_no_jank: c.config.feas_cons_no_jank,
            hold_timeout_ms: c.config.feas_hold_timeout_ms.clone(),
            release_floor_ms: c.config.feas_release_floor_ms,
            max_level: c.config.feas_max_level,
            step: c.config.feas_step,
            floor_freq: c.config.feas_floor_freq,
            release_floor_freq: c.config.feas_release_floor_freq,
            force_boost: mode.feas_force_boost,
            max_frame_us: c.config.feas_max_frame_us,
        }
    }
}
