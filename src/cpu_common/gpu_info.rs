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

use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

use crate::file_handler::FileHandler;

#[derive(Debug)]
pub struct GpuInfo {
    /// Sorted available frequencies in Hz (descending: level 0 = highest freq)
    /// Index i corresponds to power level i.
    pub freqs: Vec<isize>,
    /// Last frequency set by FAS (in Hz)
    cur_fas_freq: isize,
    verify_freq: Option<isize>,
    verify_timer: Instant,
}

impl GpuInfo {
    pub fn new() -> Result<Self> {
        // Read available frequencies from gpu_available_frequencies
        let freqs_content =
            fs::read_to_string("/sys/class/kgsl/kgsl-3d0/gpu_available_frequencies")
                .context("Failed to read gpu_available_frequencies")?;

        let mut freqs: Vec<isize> = freqs_content
            .split_whitespace()
            .map(|f| f.parse::<isize>().context("Failed to parse frequency"))
            .collect::<Result<_>>()?;

        // Sort descending so index 0 = highest frequency = power level 0
        freqs.sort_unstable_by(|a, b| b.cmp(a));

        let max_freq = *freqs.first().context("No frequencies available")?;

        Ok(Self {
            freqs,
            cur_fas_freq: max_freq,
            verify_freq: None,
            verify_timer: Instant::now(),
        })
    }

    fn verify_freq(&mut self, write_freq: isize) {
        if self.verify_timer.elapsed() >= Duration::from_secs(3) {
            self.verify_timer = Instant::now();

            if let Some(verify_freq) = self.verify_freq {
                let current_freq = self.read_freq();
                // Find the closest available frequency bounds
                let min_acceptable_freq = self
                    .freqs
                    .iter()
                    .take_while(|freq| **freq <= verify_freq)
                    .last()
                    .copied()
                    .unwrap_or(verify_freq);
                let max_acceptable_freq = self
                    .freqs
                    .iter()
                    .find(|freq| **freq >= verify_freq)
                    .copied()
                    .unwrap_or(verify_freq);

                if !(min_acceptable_freq..=max_acceptable_freq).contains(&current_freq) {
                    log::warn!(
                        "GPU: Frequency control does not meet expectations! Expected: {}-{}, Actual: {}",
                        min_acceptable_freq,
                        max_acceptable_freq,
                        current_freq
                    );
                }
            }
        }

        self.verify_freq = Some(write_freq);
    }

    /// Convert a target frequency (Hz) to the corresponding power level.
    /// Power levels are inversely mapped: level 0 = highest freq, higher level = lower freq.
    /// Returns the power level index for the highest available frequency <= target_freq.
    fn freq_to_pwrlevel(&self, target_freq: isize) -> usize {
        // Find the first frequency that is <= target_freq (since sorted descending)
        self.freqs
            .iter()
            .position(|&f| f <= target_freq)
            .unwrap_or(self.freqs.len() - 1)
    }

    /// Write frequency by setting max_pwrlevel to the level corresponding to target freq.
    pub fn write_freq(&mut self, freq: isize, file_handler: &mut FileHandler) -> Result<()> {
        let min_freq = *self.freqs.last().context("No frequencies available")?;
        let max_freq = *self.freqs.first().context("No frequencies available")?;

        let adjusted_freq = freq.clamp(min_freq, max_freq);
        self.cur_fas_freq = adjusted_freq;

        self.verify_freq(adjusted_freq);

        // Find the power level for this frequency and write to both min/max_pwrlevel
        let pwrlevel = self.freq_to_pwrlevel(adjusted_freq).to_string();
        file_handler.write_with_workround("/sys/class/kgsl/kgsl-3d0/max_pwrlevel", &pwrlevel)?;
        file_handler.write_with_workround("/sys/class/kgsl/kgsl-3d0/min_pwrlevel", &pwrlevel)
    }

    /// Reset to full available frequency range (level 0 = highest freq).
    pub fn reset(&mut self, file_handler: &mut FileHandler) -> Result<()> {
        self.verify_freq = None;

        // Level 0 = highest frequency
        file_handler.write_with_workround("/sys/class/kgsl/kgsl-3d0/max_pwrlevel", "0")?;
        file_handler.write_with_workround("/sys/class/kgsl/kgsl-3d0/min_pwrlevel", "0")
    }

    /// Read current GPU frequency from sysfs (gpuclk).
    pub fn read_freq(&self) -> isize {
        fs::read_to_string("/sys/class/kgsl/kgsl-3d0/gpuclk")
            .context("Failed to read gpuclk")
            .unwrap()
            .trim()
            .parse::<isize>()
            .context("Failed to parse gpuclk")
            .unwrap()
    }

    /// Read GPU busy percentage from sysfs
    /// Format: "23 %" → returns 0.23
    pub fn read_gpu_busy(&self) -> f64 {
        let content = fs::read_to_string("/sys/class/kgsl/kgsl-3d0/gpu_busy_percentage")
            .context("Failed to read gpu_busy_percentage")
            .unwrap_or_default();

        // Parse "23 %" format
        content
            .trim()
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .map(|v| v / 100.0)
            .unwrap_or(0.0)
    }
}
