// Copyright 2025-2025, shadow3, shadow3aaa
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

use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct ProcessMonitor {
    last_update: Instant,
    last_gpu_busy: Option<f64>,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        Self {
            last_update: Instant::now(),
            last_gpu_busy: None,
        }
    }

    pub fn set_pid(&mut self, _pid: Option<i32>) {
        // GPU busy is global, not per-process
    }

    /// Read GPU busy percentage from sysfs
    /// Returns GPU utilization as a ratio (0.0-1.0)
    pub fn update(&mut self) -> Option<f64> {
        if self.last_update.elapsed() < Duration::from_millis(300) {
            return None;
        }

        self.last_update = Instant::now();

        let gpu_busy = Self::read_gpu_busy();
        self.last_gpu_busy = Some(gpu_busy);

        Some(gpu_busy)
    }

    fn read_gpu_busy() -> f64 {
        let content = std::fs::read_to_string("/sys/class/kgsl/kgsl-3d0/gpu_busy_percentage")
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

    pub fn top_threads(&self) -> impl Iterator<Item = i32> {
        // GPU busy is not thread-specific, return empty iterator
        std::iter::empty()
    }
}
