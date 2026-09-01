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

//! FEAS (Frame-aware Energy Aware Scheduling) 决策状态机。
//!
//! 将小米内核模块 `xiaomifeas/perfmgr/perfmgr_policy.c` 的 CPU 帧调度策略
//! 移植为纯用户态算法：多级 jank 救援、预测提频、降频保持、三级超时释放。
//!
//! 核心公式（xiaomifeas 定义，M=1000）：
//! - `frame_usecs64_x_fps = 单帧μs × fps`，理想值 1e6（1 秒的微秒数）
//! - jank 线：`frame_usecs64_x_fps > 1e6 + jank_thres×1000`（默认 1.7e6 ≈ 170% 帧时长）
//! - 5 帧窗口线：`window_us × fps`，理想值 5e6
//! - 预测提频线：`> 5e6 + predict_thres×1000`（默认 5.38e6 ≈ 107.6% 单帧）
//! - 降频保持线：`< 5e6 - keepdown_thres×1000`（默认 5.05e6 ≈ 101% 单帧）
//!
//! 本模块是纯逻辑（无 IO），只产出 `FeasDecision`；频率解析在
//! `cpu_common/mod.rs::fas_update_freq_feas` 完成，职责分离、可单测。
//! `FeasParams` 定义在配置层（`crate::framework::config::FeasParams`）。

use std::time::{Duration, Instant};

use crate::framework::config::FeasParams;

/// 三级超时释放状态机（对应 xiaomifeas hrtimer 的 LIMIT→FLOOR→FLOOR_HIGH→RELEASED）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseStage {
    /// 主动提频/限频中（对应 PERFMGR_FREQUENCY_LIMIT_STATE 0x02）
    Limit,
    /// 释放天花板、保留地板（对应 RELEASE_AND_SET_FLOOR 0x03）
    Floor,
    /// 准备完全释放（对应 RELEASE_AND_SET_FLOOR_HIGH 0x04）
    FloorHigh,
    /// 全部释放（对应 FREQUENCY_RELEASED_ALL_STATE 0x01）
    Released,
}

/// 决策结果：频率档位（0=最高频）+ 当前释放阶段 + 是否卡顿。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeasDecision {
    /// 频率档位索引（0=最高频），配合 `freq_for_level` 使用
    pub freq_level: i32,
    /// 当前释放阶段
    pub stage: ReleaseStage,
    /// 是否判定为卡顿（本帧是否需要强制提频）
    pub is_janked: bool,
}

/// 每帧决策所需的上下文（纯数据，由调用方组装）。
#[derive(Debug, Clone, Copy)]
pub struct FeasContext {
    /// 当前有效帧时长（μs），已 clamp 到 `feas_max_frame_us`
    pub frame_us: u64,
    /// 当前目标 fps
    pub fps: u32,
    /// 实际可用档位上限（设备 freq 表长度-1，注入自调用方）
    pub max_level: i32,
    /// 当前时刻（用于释放阶段推进与降频冷却）
    pub now: Instant,
}

/// FEAS 决策状态机（对应 xiaomifeas 的 connected_buffer 内各计数 + 全局 last_freq_level）。
#[derive(Debug, Clone)]
pub struct FeasState {
    /// 5 帧滑动窗口（μs），新帧到来时左移
    last_frames_us: [u64; 5],
    /// 当前频率档位（0=最高频）
    last_freq_level: i32,
    /// 是否发生过卡顿（jank_happened）
    pub jank_happened: bool,
    /// 救援保持计数（rescue_keep_count）
    rescue_keep_count: i32,
    /// 连续无卡顿计数（keep_continus_count）
    keep_continus_count: i32,
    /// 救援保持总目标（rescue_keep_total_count）
    rescue_keep_total_count: i32,
    /// 快速降频计数（down_count）
    down_count: i32,
    /// 快速降频圈数（fast_down_circle）
    fast_down_circle: i32,
    /// 上次降频时刻（last_limit_time）
    last_limit_time: Instant,
    /// 当前释放阶段
    stage: ReleaseStage,
    /// 当前阶段截止时刻
    stage_deadline: Instant,
    /// 上次记录的 fps（变化时重置状态）
    last_fps: u32,
    /// 已收集的帧数（<5 时窗口未满，不做窗口类判定）
    frame_count: usize,
}

impl Default for FeasState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            last_frames_us: [0; 5],
            last_freq_level: 0,
            jank_happened: false,
            rescue_keep_count: 0,
            keep_continus_count: 0,
            rescue_keep_total_count: 0,
            down_count: 0,
            fast_down_circle: 0,
            last_limit_time: now,
            stage: ReleaseStage::Released,
            stage_deadline: now,
            last_fps: 0,
            frame_count: 0,
        }
    }
}

impl FeasState {
    /// 进入游戏/切换模式时重置（对应 init_game/模式切换路径）。
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// 是否仍处于提频/释放中的限频状态（用于决定是否走 util 省电层）。
    #[allow(dead_code)]
    pub fn in_boost(&self) -> bool {
        self.stage != ReleaseStage::Released
    }
}

/// 理想帧时长（μs）：`frame_time(fps) = 1_000_000 / fps`。
pub fn frame_time_us(fps: u32) -> u64 {
    if fps == 0 {
        0
    } else {
        1_000_000 / u64::from(fps)
    }
}

/// 档位 → 频率：level 0 = 最高频，level max_level = 最低频。
///
/// `freqs` 必须为升序离散频点。每级跨 `step` 个频点（level 索引映射到
/// 原数组索引 `(len-1) - level×step`）。越界自动 clamp。
/// 此函数供 `fas_update_freq_feas` 在写频侧使用。
pub fn freq_for_level(freqs: &[isize], level: i32, step: usize) -> isize {
    if freqs.is_empty() {
        return 0;
    }
    let step = step.max(1) as i32;
    let max_index = freqs.len() as i32 - 1;
    let max_level = max_index / step;
    let idx = max_index - level.clamp(0, max_level) * step;
    freqs[idx.clamp(0, max_index) as usize]
}

/// 有效档位上限：config `feas_max_level` 覆盖，否则用设备注入的档位数。
fn effective_max_level(p: &FeasParams, ctx_max_level: i32) -> i32 {
    if p.max_level > 0 {
        p.max_level
    } else {
        ctx_max_level.max(0)
    }
}

/// 快速降频圈数自适应（对应 xiaomifeas `update_circle`）。
fn update_circle(circle: i32, last_freq_level: i32, max_level: i32) -> i32 {
    if max_level <= 0 {
        return circle;
    }
    let m = max_level;
    if last_freq_level < m / 3 {
        circle - circle / 3
    } else if last_freq_level < m / 2 {
        circle - circle / 4
    } else if last_freq_level > m * 2 / 3 {
        circle
    } else {
        circle + circle / 4
    }
}

/// 提频保持时长：按 fps 查表，缺省回退 25000ms（60fps 默认）。
fn hold_timeout(fps: u32, p: &FeasParams) -> Duration {
    let ms = p.hold_timeout_ms.get(&fps).copied().unwrap_or(25_000);
    Duration::from_millis(u64::from(ms))
}

/// 降频冷却是否已满足：`Δt(μs) × fps > cooldown × 1e6`
/// （对应 xiaomifeas `(current_time - last_limit_time) * fps > scaling_c * M * M`，
/// 内核按 ns 计实为恒真 bug，此处按 μs 语义修正。）
fn cooldown_elapsed(last: Instant, now: Instant, fps: u32, p: &FeasParams) -> bool {
    let dt_us = now
        .checked_duration_since(last)
        .unwrap_or(Duration::ZERO)
        .as_micros() as f64;
    dt_us * f64::from(fps) > f64::from(p.keepdown_cooldown) * 1_000_000.0
}

/// 五级救援档位：`frame_us_x_fps > 1e6 + n×rescue_step×1e3` → level n。
fn rescue_level(frame_us_x_fps: f64, step: i32) -> i32 {
    let step = f64::from(step.max(1));
    for n in (1..=5).rev() {
        if frame_us_x_fps > 1_000_000.0 + step * 1000.0 * f64::from(n) {
            return n;
        }
    }
    1
}

/// 推入一帧：窗口左移（对应 left_shift + last_frame_unit[FRAME_UNIT-1]=frame）。
fn push_frame(state: &mut FeasState, frame_us: u64) {
    for i in 0..4 {
        state.last_frames_us[i] = state.last_frames_us[i + 1];
    }
    state.last_frames_us[4] = frame_us;
    if state.frame_count < 5 {
        state.frame_count += 1;
    }
}

/// 推进释放阶段（对应 hrtimer 状态机，timeout 检查）。
///
/// - Limit 阶段到期：`last_freq_level` 微降（对应 timeout_r_freq_level=2），进入 Floor
/// - Floor 到期：进入 FloorHigh
/// - FloorHigh 到期：Released
/// 新提频会由 jank/predict 分支把 stage 重置回 Limit。
fn advance_stage(state: &mut FeasState, now: Instant, p: &FeasParams, ctx_max_level: i32) {
    if now < state.stage_deadline {
        return;
    }

    match state.stage {
        ReleaseStage::Limit => {
            // timeout_r_freq_level=2：超时后降 2 档再释放
            let max = effective_max_level(p, ctx_max_level);
            state.last_freq_level = (state.last_freq_level - 2).clamp(0, max);
            state.stage = ReleaseStage::Floor;
            state.stage_deadline = now + Duration::from_millis(u64::from(p.release_floor_ms));
        }
        ReleaseStage::Floor => {
            state.stage = ReleaseStage::FloorHigh;
            state.stage_deadline = now + Duration::from_millis(u64::from(p.release_floor_ms));
        }
        ReleaseStage::FloorHigh => {
            state.stage = ReleaseStage::Released;
            state.last_freq_level = 0;
        }
        ReleaseStage::Released => (),
    }
}

/// 每帧决策入口（纯函数，对应 `perfmgr_do_policy`）。
///
/// 调用约定：每次 `do_policy` 调用一次；帧数据来自 Buffer（含兜底帧）。
/// 若 `ctx.frame_us` 为 0（无有效帧），本函数只推进释放阶段，不做帧判定。
///
/// 档位语义：`last_freq_level` 越小频率越高（0=最高频）。jank 救援 level 为正，
/// 使档位向 0（更高频）靠近；降频 level 为负，使档位增大（更低频）。
pub fn calculate_decision(state: &mut FeasState, ctx: &FeasContext, p: &FeasParams) -> FeasDecision {
    // fps 变化 → 重置（对应 xiaomifeas 目标 fps 变化时清窗口）
    if ctx.fps != state.last_fps {
        state.reset();
        state.last_fps = ctx.fps;
    }

    // 先推进释放阶段（等效 hrtimer 回调检查）
    advance_stage(state, ctx.now, p, ctx.max_level);

    // 无有效帧数据：保持当前阶段，不做帧判定
    if ctx.frame_us == 0 {
        return FeasDecision {
            freq_level: state.last_freq_level,
            stage: state.stage,
            is_janked: state.stage == ReleaseStage::Limit,
        };
    }

    // 喂入滑动窗口（对应 left_shift + last_frame_unit[FRAME_UNIT-1]）
    push_frame(state, ctx.frame_us);

    let frame_us_x_fps = ctx.frame_us as f64 * f64::from(ctx.fps);
    let jank_thres = 1_000_000.0 + f64::from(p.jank_thres) * 1000.0;

    let mut level = 0;
    let mut r_p_level = 0;
    let mut set_freq = false;

    // ---- jank 救援分支（单帧判定，无需窗口填满；对应 perfmgr_policy.c:666-696） ----
    if frame_us_x_fps > jank_thres {
        state.jank_happened = true;
        state.rescue_keep_count = 0;
        state.keep_continus_count = 0;
        state.stage = ReleaseStage::Limit;
        state.stage_deadline = ctx.now + hold_timeout(ctx.fps, p);

        if p.rescue_perf {
            // r_perf=1：五级救援（阈值 1e6 + n×rescue_step×1e3）
            level = rescue_level(frame_us_x_fps, p.rescue_step);
            r_p_level = 0;
        } else if p.rescue_step > 300
            && frame_us_x_fps > jank_thres + f64::from(p.rescue_step) * 1000.0
        {
            // r_perf=0：两级救援，超过 r_step 额外升一级
            r_p_level = 1;
            level = 1;
        } else {
            r_p_level = 0;
            level = 1;
        }
        set_freq = true;
    } else {
        state.jank_happened = false;
        state.keep_continus_count += 1;
    }

    // ---- 窗口类分支（predict/keepdown）需窗口填满 ----
    if !set_freq && state.frame_count >= 5 {
        let window_us = state.last_frames_us.iter().sum::<u64>();
        let window_us_x_fps = window_us as f64 * f64::from(ctx.fps);
        let predict_thres = 5_000_000.0 + f64::from(p.predict_thres) * 1000.0;
        let keepdown_thres = 5_000_000.0 - f64::from(p.keepdown_thres) * 1000.0;

        // ---- 预测提频分支（对应 perfmgr_policy.c:739-764） ----
        if window_us_x_fps > predict_thres {
            if p.predict_perf {
                // p_perf=1：三级
                if window_us_x_fps > predict_thres + 2.0 * f64::from(p.predict_step) * 1000.0 {
                    level = 3;
                } else if window_us_x_fps > predict_thres + f64::from(p.predict_step) * 1000.0 {
                    level = 2;
                } else {
                    level = 1;
                }
                r_p_level = 0;
            } else if window_us_x_fps > predict_thres + f64::from(p.predict_step) * 1000.0 {
                // p_perf=0：两级
                r_p_level = 1;
                level = 2;
            } else {
                r_p_level = 1;
                level = 1;
            }
            state.stage = ReleaseStage::Limit;
            state.stage_deadline = ctx.now + hold_timeout(ctx.fps, p);
            set_freq = true;
        }
        // ---- 降频保持分支（对应 perfmgr_policy.c:700-737） ----
        else if window_us_x_fps < keepdown_thres {
            state.rescue_keep_count += 1;
            state.rescue_keep_total_count = if state.jank_happened {
                p.nor_keep + p.jank_keep
            } else {
                p.nor_keep
            };

            let count_ok = state.rescue_keep_count >= state.rescue_keep_total_count
                || (p.cons_no_jank > 0 && state.keep_continus_count > p.cons_no_jank);

            if count_ok && cooldown_elapsed(state.last_limit_time, ctx.now, ctx.fps, p) {
                state.jank_happened = false;
                state.rescue_keep_count = 0;
                state.keep_continus_count = 0;
                state.last_limit_time = ctx.now;

                let max = effective_max_level(p, ctx.max_level);
                state.fast_down_circle =
                    update_circle(p.step.max(1) as i32, state.last_freq_level, max);
                state.down_count += 1;
                if state.fast_down_circle > 0 && state.down_count % state.fast_down_circle == 1 {
                    level = -2; // fast_down_freq_level
                    state.down_count = 1;
                } else {
                    level = -1;
                }
                set_freq = true;
            }
        }
    }

    if set_freq {
        // set_freq_level = last_freq_level - r_p_level - level
        let max = effective_max_level(p, ctx.max_level);
        let target = state.last_freq_level - r_p_level - level;
        state.last_freq_level = target.clamp(0, max);

        // load_reset：提频后把窗口末槽重置为近理想帧，使 5 帧窗口偏向"已恢复"
        if level >= 0 {
            let ideal = frame_time_us(ctx.fps);
            state.last_frames_us[4] = ideal.saturating_sub(ideal >> 10);
        }
    }

    FeasDecision {
        freq_level: state.last_freq_level,
        stage: state.stage,
        is_janked: state.stage == ReleaseStage::Limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 60fps 下的"中立帧"：5 帧窗口=85_000μs ×60 = 5.1e6，落在中立带
    /// （keepdown 线 5.05e6 与 predict 线 5.38e6 之间）。
    const NEUTRAL_FRAME_US: u64 = 17_000;
    /// 测试用档位上限
    const TEST_MAX_LEVEL: i32 = 27;

    fn ctx(frame_us: u64, fps: u32, now: Instant) -> FeasContext {
        FeasContext {
            frame_us,
            fps,
            max_level: TEST_MAX_LEVEL,
            now,
        }
    }

    fn default_params() -> FeasParams {
        FeasParams::default_params()
    }

    /// 构造一个档位基线已降、fps 已锁定的状态（供救援/预测测试播种）。
    fn seeded_state(level: i32) -> FeasState {
        let mut state = FeasState::default();
        state.last_fps = 60;
        state.last_freq_level = level;
        state
    }

    /// 连续喂 N 帧中立帧（每帧 now 递增 17ms），返回最新 now。不断言档位。
    fn feed_neutral(state: &mut FeasState, n: usize, mut now: Instant) -> Instant {
        for _ in 0..n {
            now += Duration::from_millis(17);
            let _ = calculate_decision(state, &ctx(NEUTRAL_FRAME_US, 60, now), &default_params());
        }
        now
    }

    #[test]
    fn frame_time_us_ideal() {
        assert_eq!(frame_time_us(60), 16_666);
        assert_eq!(frame_time_us(120), 8333);
        assert_eq!(frame_time_us(30), 33_333);
        assert_eq!(frame_time_us(0), 0);
    }

    #[test]
    fn freq_for_level_mapping() {
        let freqs = [100, 200, 300, 400, 500];
        assert_eq!(freq_for_level(&freqs, 0, 1), 500); // level 0 = 最高频
        assert_eq!(freq_for_level(&freqs, 4, 1), 100); // max_level = 最低频
        assert_eq!(freq_for_level(&freqs, 99, 1), 100); // 越界 clamp 到最低
        assert_eq!(freq_for_level(&freqs, -5, 1), 500); // 负越界 clamp 到最高
        assert_eq!(freq_for_level(&freqs, 2, 1), 300);
        assert_eq!(freq_for_level(&freqs, 1, 2), 300); // step=2：idx = 4 - 1×2 = 2
        assert_eq!(freq_for_level(&[], 0, 1), 0); // 空表
    }

    #[test]
    fn jank_threshold_boundary() {
        // 60fps，jank 线 = 1e6 + 700e3 = 1.7e6，单帧阈值 = 1.7e6/60 ≈ 28333μs
        let mut state = seeded_state(5);
        let now = Instant::now();

        // 恰好在线下（28_000μs → 1.68e6 < 1.7e6）：不触发 jank，档位保持
        let d = calculate_decision(&mut state, &ctx(28_000, 60, now), &default_params());
        assert!(!d.is_janked);
        assert_eq!(d.stage, ReleaseStage::Released);
        assert_eq!(d.freq_level, 5);

        // 超过线（28_500μs → 1.71e6 > 1.7e6）：触发 jank，进 Limit，五级救援 level1
        let now = now + Duration::from_millis(16);
        let d = calculate_decision(&mut state, &ctx(28_500, 60, now), &default_params());
        assert!(d.is_janked);
        assert_eq!(d.stage, ReleaseStage::Limit);
        assert_eq!(d.freq_level, 4); // 5 - 1
    }

    #[test]
    fn rescue_levels_1_to_5() {
        // r_perf=1，step=750：阈值 1e6 + n×750e3
        // n=1: 1.75e6, n=2: 2.5e6, n=3: 3.25e6, n=4: 4.0e6, n=5: 4.75e6
        // 60fps 下对应帧时长：29167, 41667, 54167, 66667, 79167 μs
        let cases = [
            (30_000u64, 1), // 1.8e6 > 1.75e6 → level1 → 档位 10-1=9
            (42_000, 2),    // 2.52e6 > 2.5e6 → level2 → 8
            (55_000, 3),    // 3.3e6 > 3.25e6 → level3 → 7
            (67_000, 4),    // 4.02e6 > 4.0e6 → level4 → 6
            (80_000, 5),    // 4.8e6 > 4.75e6 → level5 → 5
        ];
        for (frame_us, expect_delta) in cases {
            let mut state = seeded_state(10);
            let now = Instant::now();
            let d = calculate_decision(&mut state, &ctx(frame_us, 60, now), &default_params());
            assert_eq!(d.freq_level, 10 - expect_delta, "frame_us={frame_us}");
            assert_eq!(d.stage, ReleaseStage::Limit);
        }
    }

    #[test]
    fn predict_two_level_boost() {
        // p_perf=false：predict 线 = 5e6 + 380e3 = 5.38e6；二级线 = 5.38e6 + 750e3 = 6.13e6
        // 5 帧均匀 19_000μs → 窗口 95_000 ×60 = 5.7e6（一级），单帧 1.14e6 < 1.7e6 不触发 jank
        let p = default_params();
        let mut state = seeded_state(10);
        let mut now = Instant::now();
        for _ in 0..4 {
            now += Duration::from_millis(19);
            let _ = calculate_decision(&mut state, &ctx(19_000, 60, now), &p);
        }
        // 第 5 帧触发 predict：target = 10 - r_p(1) - level(1) = 8
        now += Duration::from_millis(19);
        let d = calculate_decision(&mut state, &ctx(19_000, 60, now), &p);
        assert_eq!(d.stage, ReleaseStage::Limit);
        assert_eq!(d.freq_level, 8, "predict level1");
    }

    #[test]
    fn predict_two_level_second_tier() {
        // 5 帧均匀 20_600μs → 窗口 103_000 ×60 = 6.18e6 > 6.13e6（二级）
        // 单帧 1.236e6 < 1.7e6 不触发 jank
        // target = 10 - r_p(1) - level(2) = 7
        let p = default_params();
        let mut state = seeded_state(10);
        let mut now = Instant::now();
        for _ in 0..4 {
            now += Duration::from_millis(21);
            let _ = calculate_decision(&mut state, &ctx(20_600, 60, now), &p);
        }
        // 第 5 帧触发二级 predict
        now += Duration::from_millis(21);
        let d = calculate_decision(&mut state, &ctx(20_600, 60, now), &p);
        assert_eq!(d.freq_level, 7);
    }

    #[test]
    fn keepdown_blocked_by_cooldown() {
        // 冷却：cooldown=3 → Δt×60 > 3e6 → Δt > 50ms
        let p = default_params();
        let mut state = FeasState::default();
        let mut now = Instant::now();

        // 填 5 帧快速帧（窗口 5×16000 = 80000μs ×60 = 4.8e6 < 5.05e6 → 进 keepdown 分支）
        for _ in 0..5 {
            now += Duration::from_millis(16);
            let _ = calculate_decision(&mut state, &ctx(16_000, 60, now), &p);
        }
        // 冷却未满足（last_limit_time 距 now < 50ms）：不应降频
        now += Duration::from_millis(1);
        let d = calculate_decision(&mut state, &ctx(16_000, 60, now), &p);
        assert_eq!(d.freq_level, 0, "cooldown should block keepdown");
    }

    #[test]
    fn keepdown_after_cooldown_and_cons() {
        // 连续无卡顿帧 > cons_no_jank(10) 且冷却满足 → 降频
        let p = default_params();
        let mut state = FeasState::default();
        let mut now = Instant::now();

        // 先把 last_limit_time 老化 > 50ms
        now += Duration::from_millis(100);

        let mut triggered = false;
        for _ in 0..40 {
            now += Duration::from_millis(16);
            let d = calculate_decision(&mut state, &ctx(16_000, 60, now), &p);
            if d.freq_level > 0 {
                triggered = true;
                break;
            }
        }
        assert!(triggered, "keepdown should trigger after cooldown + cons_no_jank");
    }

    #[test]
    fn three_stage_release_state_machine() {
        // Limit → (hold=25s@60fps) → Floor → (2333ms) → FloorHigh → (2333ms) → Released
        // 用 frame_us=0 的决策推进时间（只走阶段机，不喂帧，避免 predict 重新武装）
        let p = default_params();
        let mut state = FeasState::default();
        let mut now = Instant::now();

        // 触发一次 jank 进入 Limit
        let d = calculate_decision(&mut state, &ctx(30_000, 60, now), &p);
        assert_eq!(d.stage, ReleaseStage::Limit);

        // hold 未到期（10s < 25s）：仍在 Limit
        now += Duration::from_millis(10_000);
        let d = calculate_decision(&mut state, &ctx(0, 60, now), &p);
        assert_eq!(d.stage, ReleaseStage::Limit);

        // 超过 hold（25s）：进入 Floor
        now += Duration::from_millis(20_000);
        let d = calculate_decision(&mut state, &ctx(0, 60, now), &p);
        assert_eq!(d.stage, ReleaseStage::Floor);

        // 再过 2333ms：进入 FloorHigh
        now += Duration::from_millis(2500);
        let d = calculate_decision(&mut state, &ctx(0, 60, now), &p);
        assert_eq!(d.stage, ReleaseStage::FloorHigh);

        // 再过 2333ms：Released
        now += Duration::from_millis(2500);
        let d = calculate_decision(&mut state, &ctx(0, 60, now), &p);
        assert_eq!(d.stage, ReleaseStage::Released);
    }

    #[test]
    fn new_jank_resets_to_limit() {
        // 在 Floor 阶段遇到新 jank 应重置回 Limit
        let p = default_params();
        let mut state = FeasState::default();
        let mut now = Instant::now();

        // jank 进入 Limit，然后等超时进入 Floor（用 frame_us=0 只推时间）
        let _ = calculate_decision(&mut state, &ctx(30_000, 60, now), &p);
        now += Duration::from_millis(26_000);
        let d = calculate_decision(&mut state, &ctx(0, 60, now), &p);
        assert_eq!(d.stage, ReleaseStage::Floor);

        // 新 jank → 重置回 Limit
        now += Duration::from_millis(1);
        let d = calculate_decision(&mut state, &ctx(30_000, 60, now), &p);
        assert_eq!(d.stage, ReleaseStage::Limit);
        assert!(d.is_janked);
    }

    #[test]
    fn fps_change_resets_state() {
        // fps 变化应重置窗口与档位
        let p = default_params();
        let mut state = FeasState::default();
        let now = Instant::now();

        // 120fps 下 jank（帧 15_000μs → 1.8e6 > 1.7e6；120fps jank 线 1.7e6/120≈14167μs）
        let d = calculate_decision(&mut state, &ctx(15_000, 120, now), &p);
        assert_eq!(d.stage, ReleaseStage::Limit);

        // 切回 60fps：重置，窗口清空，中立帧（窗口未满守卫）不应沿用 120fps 的档位
        let now = now + Duration::from_millis(17);
        let d = calculate_decision(&mut state, &ctx(NEUTRAL_FRAME_US, 60, now), &p);
        assert_eq!(d.freq_level, 0, "fps change must reset freq_level");
        assert_eq!(d.stage, ReleaseStage::Released);
    }

    #[test]
    fn max_level_clamp() {
        // 降频过深 + max_level 覆盖：档位应被 clamp 到 max_level
        let p = FeasParams {
            max_level: 10,
            ..default_params()
        };
        // 先把档位播种到 20（超过 max_level），再触发一次降频后的救援，应 clamp 到 ≤10
        let mut state = seeded_state(20);
        let now = Instant::now();
        let d = calculate_decision(&mut state, &ctx(30_000, 60, now), &p);
        assert!(d.freq_level <= 10, "freq_level must clamp to max_level");
        // 极端 jank：5 级救援也从 20 起算，clamp 到 10
        let d = calculate_decision(&mut state, &ctx(80_000, 60, now), &p);
        assert!(d.freq_level <= 10);
    }
}
