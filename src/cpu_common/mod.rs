// Copyright 2023-2025, dependabot[bot], shadow3, shadow3aaa
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

pub mod extra_policy;
mod gpu_info;
mod process_monitor;

use std::{sync::OnceLock, thread, time::Duration};

use anyhow::{Context, Result};
#[cfg(debug_assertions)]
use log::debug;
use log::warn;
use process_monitor::ProcessMonitor;

use crate::{
    Extension,
    api::{trigger_init_cpu_freq, trigger_reset_cpu_freq},
    file_handler::FileHandler,
};
use gpu_info::GpuInfo;

pub static EXTRA_POLICY_MAP: OnceLock<()> = OnceLock::new();

/// Ensure GPU powerlevel files are writable before use.
fn ensure_gpu_writable() {
    let _ = std::process::Command::new("chmod")
        .args(["777", "/sys/class/kgsl/kgsl-3d0/min_pwrlevel"])
        .output();
    let _ = std::process::Command::new("chmod")
        .args(["777", "/sys/class/kgsl/kgsl-3d0/max_pwrlevel"])
        .output();
}

#[derive(Debug)]
pub struct Controller {
    max_freq: isize,
    gpu_info: GpuInfo,
    file_handler: FileHandler,
    process_monitor: ProcessMonitor,
    util_max: Option<f64>,
}

impl Controller {
    pub fn new() -> Result<Self> {
        ensure_gpu_writable();
        let gpu_info = Self::load_gpu_info()?;

        #[cfg(debug_assertions)]
        debug!(
            "gpu info: max_freq={}",
            gpu_info.freqs.first().unwrap_or(&0)
        );

        // Index 0 = highest frequency (power level 0)
        let max_freq = *gpu_info.freqs.first().context("No frequencies available")?;

        Ok(Self {
            max_freq,
            gpu_info,
            file_handler: FileHandler::new(),
            process_monitor: ProcessMonitor::new(),
            util_max: None,
        })
    }

    fn load_gpu_info() -> Result<GpuInfo> {
        loop {
            match GpuInfo::new() {
                Ok(info) => return Ok(info),
                Err(e) => {
                    warn!("Failed to read GPU info, reason: {e:?}",);
                    warn!("Retrying...");
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }

    pub fn init_game(&mut self, pid: i32, extension: &Extension) {
        trigger_init_cpu_freq(extension);
        self.reset_all_gpu_freq();
        self.process_monitor.set_pid(Some(pid));
        self.util_max = None;
    }

    pub fn init_default(&mut self, extension: &Extension) {
        trigger_reset_cpu_freq(extension);
        self.reset_all_gpu_freq();
        self.process_monitor.set_pid(None);
        self.util_max = None;
    }

    pub fn fas_update_freq(&mut self, control: isize, is_janked: bool) {
        #[cfg(debug_assertions)]
        debug!("change freq: {control}");

        // Clamp control to prevent runaway accumulation from the PID controller.
        // Without this, a persistent frametime error (e.g. target 30fps vs actual 20fps)
        // causes control to grow indefinitely each frame → pushes to max_freq then drops to min_freq.
        const MAX_CONTROL_DELTA: isize = 1_000_000_000; // ±1 GHz per frame
        let clamped_control = control.clamp(-MAX_CONTROL_DELTA, MAX_CONTROL_DELTA);

        let target_freq = self.compute_target_frequency(clamped_control, is_janked);
        // Floor: never go below 25% of max available frequency
        let min_floor = (self.max_freq / 4).max(160_000_000); // at least 160 MHz
        let target_freq = target_freq.clamp(min_floor, self.max_freq);
        let _ = self
            .gpu_info
            .write_freq(target_freq, &mut self.file_handler);
    }

    fn update_util_max(&mut self) {
        if let Some(util_max) = self.process_monitor.update() {
            self.util_max = Some(util_max);
        }
    }

    fn compute_target_frequency(&mut self, control: isize, is_janked: bool) -> isize {
        let cur_fas_freq = self.gpu_info.cur_fas_freq;
        let cur_freq = self.gpu_info.read_freq();

        if is_janked {
            self.util_max = None;
        } else {
            self.update_util_max();
        }

        if is_janked || self.util_max.is_none() {
            // No utilization data or janked state: use raw control value
            cur_fas_freq.saturating_add(control).clamp(0, self.max_freq)
        } else {
            // Utilization-based scaling: if GPU util < 50%, scale down proportionally
            let util_tracking_sugg_freq = (cur_freq as f64 * self.util_max.unwrap() / 0.5) as isize; // min_util: 50%

            #[cfg(debug_assertions)]
            debug!(
                "util: {}, cur_freq: {}, util_tracking_sugg_freq: {}",
                self.util_max.unwrap(),
                cur_freq,
                util_tracking_sugg_freq
            );

            cur_fas_freq
                .saturating_add(control)
                .min(util_tracking_sugg_freq)
                .clamp(0, self.max_freq)
        }
    }

    fn reset_all_gpu_freq(&mut self) {
        let _ = self.gpu_info.reset(&mut self.file_handler);
    }

    pub fn util_max(&self) -> f64 {
        self.util_max.unwrap_or_default()
    }
}
