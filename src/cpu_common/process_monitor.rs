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
use std::{
    cmp,
    collections::{HashMap, HashSet},
    fs,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
#[cfg(debug_assertions)]
use log::debug;
use libc::{_SC_CLK_TCK, sysconf};

#[derive(Debug, Clone)]
struct UsageTracker {
    pid: i32,
    tid: i32,
    last_cputime: u64,
    read_timer: Instant,
    current_usage: f64,
    valid: bool,
}

impl UsageTracker {
    fn new(pid: i32, tid: i32) -> Result<Self> {
        Ok(Self {
            pid,
            tid,
            last_cputime: get_thread_cpu_time(pid, tid)?,
            read_timer: Instant::now(),
            current_usage: 0.0,
            valid: false,
        })
    }

    fn try_calculate(&mut self) -> Option<f64> {
        let tick_per_sec = unsafe { sysconf(_SC_CLK_TCK) };
        let new_cputime = get_thread_cpu_time(self.pid, self.tid).ok()?;
        let elapsed = self.read_timer.elapsed();
        self.read_timer = Instant::now();

        if !self.valid {
            self.last_cputime = new_cputime;
            self.valid = true;
            return None;
        }

        let elapsed_ticks = elapsed.as_secs_f64() * tick_per_sec as f64;
        if elapsed_ticks < 1.0 {
            self.last_cputime = new_cputime;
            return None;
        }

        let cputime_slice = new_cputime.saturating_sub(self.last_cputime);
        self.last_cputime = new_cputime;
        self.current_usage = cputime_slice as f64 / elapsed_ticks;
        Some(self.current_usage)
    }
}

#[derive(Debug)]
pub struct ProcessMonitor {
    current_pid: Option<i32>,
    trackers: HashMap<i32, UsageTracker>,
    top_threads_cache: Vec<i32>,
    last_full_update: Instant,
    last_update: Instant,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        Self {
            current_pid: None,
            trackers: HashMap::new(),
            top_threads_cache: Vec::new(),
            last_full_update: Instant::now(),
            last_update: Instant::now(),
        }
    }

    pub fn set_pid(&mut self, pid: Option<i32>) {
        if self.current_pid != pid {
            self.current_pid = pid;
            self.trackers.clear();
            self.top_threads_cache.clear();
            self.last_full_update = Instant::now();
            self.last_update = Instant::now();
        }
    }

    pub fn update(&mut self) -> Option<f64> {
        if self.last_update.elapsed() < Duration::from_millis(300) {
            return None;
        }

        self.last_update = Instant::now();
        let pid = self.current_pid?;

        if self.last_full_update.elapsed() >= Duration::from_secs(1) {
            self.update_thread_list(pid);
            self.last_full_update = Instant::now();
        }

        let mut util_max: f64 = 0.0;
        let mut has_valid_usage = false;
        let tid_list: Vec<i32> = self.top_threads_cache.clone();
        for tid in tid_list {
            if let Some(tracker) = self.trackers.get_mut(&tid) {
                if let Some(usage) = tracker.try_calculate() {
                    util_max = util_max.max(usage);
                    has_valid_usage = true;
                }
            }
        }

        #[cfg(debug_assertions)]
        debug!(
            "process_monitor: top_threads={}, util_max={:.3}",
            self.top_threads_cache.len(),
            util_max
        );

        if has_valid_usage {
            Some(util_max)
        } else {
            None
        }
    }

    fn update_thread_list(&mut self, pid: i32) {
        let Ok(threads) = get_thread_ids(pid) else {
            return;
        };

        let current_tids: HashSet<i32> = threads.iter().copied().collect();

        self.trackers.retain(|tid, _| current_tids.contains(tid));

        for tid in &threads {
            self.trackers
                .entry(*tid)
                .or_insert_with(|| UsageTracker::new(pid, *tid).unwrap_or_else(|_| {
                    UsageTracker {
                        pid,
                        tid: *tid,
                        last_cputime: 0,
                        read_timer: Instant::now(),
                        current_usage: 0.0,
                        valid: false,
                    }
                }));
        }

        let mut thread_usages: Vec<(i32, f64)> = Vec::new();
        for (tid, tracker) in self.trackers.iter_mut() {
            if let Some(usage) = tracker.try_calculate() {
                thread_usages.push((*tid, usage));
            }
        }

        thread_usages.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(cmp::Ordering::Equal));
        thread_usages.truncate(8);

        self.top_threads_cache = thread_usages.iter().map(|(tid, _)| *tid).collect();
    }

    pub fn top_threads(&self) -> impl Iterator<Item = i32> + '_ {
        self.top_threads_cache.iter().copied()
    }
}

fn get_thread_ids(pid: i32) -> Result<Vec<i32>> {
    let proc_path = format!("/proc/{pid}/task");
    Ok(fs::read_dir(proc_path)?
        .filter_map(|entry| {
            entry
                .ok()
                .and_then(|e| e.file_name().to_string_lossy().parse::<i32>().ok())
        })
        .collect())
}

fn get_thread_cpu_time(pid: i32, tid: i32) -> Result<u64> {
    let stat_path = format!("/proc/{pid}/task/{tid}/stat");
    let stat_content = fs::read_to_string(&stat_path)
        .with_context(|| format!("Failed to read {}", stat_path))?;

    let first_paren = stat_content
        .find('(')
        .context("Failed to find '(' in stat")?;
    let last_paren = stat_content
        .rfind(')')
        .context("Failed to find ')' in stat")?;

    let after_comm = &stat_content[last_paren + 1..];
    let fields: Vec<&str> = after_comm.split_whitespace().collect();

    if fields.len() < 13 {
        return Ok(0);
    }

    let utime: u64 = fields[11].parse().unwrap_or(0);
    let stime: u64 = fields[12].parse().unwrap_or(0);

    Ok(utime + stime)
}
