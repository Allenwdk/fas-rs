// Copyright 2024-2025, shadow3, shadow3aaa
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

use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(debug_assertions)]
use log::debug;
use log::warn;

use crate::cpu_common::EXTRA_POLICY_MAP;

static WARNING_FLAG: AtomicBool = AtomicBool::new(false);

/// No-op: GPU调频不再使用extra policy
pub fn remove_extra_policy(_policy: i32) {
    let _ = EXTRA_POLICY_MAP.get_or_init(|| ());
}

/// No-op: GPU调频不再使用extra policy
pub fn set_extra_policy_abs(_policy: i32, _min: Option<isize>, _max: Option<isize>) {
    let _ = EXTRA_POLICY_MAP.get_or_init(|| ());
    #[cfg(debug_assertions)]
    debug!("EXTRA_POLICY_MAP (no-op): {:?}", EXTRA_POLICY_MAP.get());
}

/// No-op: GPU调频不再使用extra policy
pub fn set_extra_policy_rel(
    _policy: i32,
    _target_policy: i32,
    _min: Option<isize>,
    _max: Option<isize>,
) {
    let _ = EXTRA_POLICY_MAP.get_or_init(|| ());
    #[cfg(debug_assertions)]
    debug!("EXTRA_POLICY_MAP (no-op): {:?}", EXTRA_POLICY_MAP.get());
}

pub fn set_policy_freq_offset(_: i32, _: isize) {
    if !WARNING_FLAG.load(Ordering::Acquire) {
        warn!(
            "The API set_policy_freq_offset was removed in v4.2.0. If you see this warning, it means an outdated plugin is trying to use it. The warning will only appear once."
        );
        WARNING_FLAG.store(true, Ordering::Release);
    }
}

/// No-op: GPU调频不再使用ignore policy
pub fn set_ignore_policy(_policy: i32, _val: bool) {
    // GPU busy is global, not per-policy
}
