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
// You should have received a copy of the General Public License along
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
    pub path: PathBuf,
    /// Sorted list of all available GPU frequencies (Hz)
    pub freqs: Vec<isize>,
    /// Last frequency set by FAS (in Hz)
    pub cur_fas_freq: isize,
    verify_freq: Option<isize>,
    verify_timer: Instant,
}

impl GpuInfo {
    pub fn new() -> Result<Self> {
        let path = PathBuf::from("/sys/class/kgsl/kgsl-3d0/devfreq");

        // Read available frequencies
        let freqs_content = fs::read_to_string(path.join("scaling_available_frequencies"))
            .context("Failed to read scaling_available_frequencies")?;

        let mut freqs: Vec<isize> = freqs_content
            .split_whitespace()
            .map(|f| f.parse::<isize>().context("Failed to parse frequency"))
            .collect::<Result<_>>()?;

        freqs.sort_unstable();

        let max_freq = *freqs.last().context("No frequencies available")?;

        Ok(Self {
            path,
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

    /// Write frequency to max_freq (clamp to available range)
    pub fn write_freq(&mut self, freq: isize, file_handler: &mut FileHandler) -> Result<()> {
        let min_freq = *self.freqs.first().context("No frequencies available")?;
        let max_freq = *self.freqs.last().context("No frequencies available")?;

        let adjusted_freq = freq.clamp(min_freq, max_freq);
        self.cur_fas_freq = adjusted_freq;

        self.verify_freq(adjusted_freq);

        file_handler.write_with_workround(self.max_freq_path(), &adjusted_freq.to_string())
    }

    /// Reset to full available frequency range
    pub fn reset(&mut self, file_handler: &mut FileHandler) -> Result<()> {
        let min_freq = *self.freqs.first().context("No frequencies available")?;
        let max_freq = *self.freqs.last().context("No frequencies available")?;

        self.verify_freq = None;

        file_handler.write_with_workround(self.max_freq_path(), &max_freq.to_string())?;
        file_handler.write_with_workround(self.min_freq_path(), &min_freq.to_string())
    }

    /// Read current GPU frequency from sysfs
    pub fn read_freq(&self) -> isize {
        fs::read_to_string(self.path.join("scaling_cur_freq"))
            .context("Failed to read scaling_cur_freq")
            .unwrap()
            .trim()
            .parse::<isize>()
            .context("Failed to parse scaling_cur_freq")
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

    fn max_freq_path(&self) -> PathBuf {
        self.path.join("max_freq")
    }

    fn min_freq_path(&self) -> PathBuf {
        self.path.join("min_freq")
    }
}
