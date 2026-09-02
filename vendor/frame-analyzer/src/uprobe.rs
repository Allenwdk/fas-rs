/*
 * Copyright (c) 2024 shadow3aaa@gitbub.com
 *
 * This file is part of frame-analyzer-ebpf.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
use aya::{
    Ebpf,
    maps::{MapData, RingBuf},
    programs::UProbe,
};

use crate::{ebpf::load_bpf, error::{AnalyzerError, Result}};

pub struct UprobeHandler {
    bpf: Ebpf,
}

impl Drop for UprobeHandler {
    fn drop(&mut self) {
        if let Ok(program) = self.get_program() {
            let _ = program.unload();
        }
    }
}

impl UprobeHandler {
    pub fn attach_app(pid: i32) -> Result<Self> {
        const LIBGUI: &str = "/system/lib64/libgui.so";
        const SYMBOLS: &[&str] = &[
            "_ZN7android7Surface11queueBufferEP19ANativeWindowBufferi",
            "_ZN7android7Surface11queueBufferEP19ANativeWindowBufferiPNS_24SurfaceQueueBufferOutputE",
            "_ZN7android7Surface11queueBufferERKNS_2spINS_13GraphicBufferEEERKNS1_INS_5FenceEEEPNS_24SurfaceQueueBufferOutputE",
            "_ZN7android7Surface11queueBufferERKNS_2spINS_13GraphicBufferEEERKNS1_INS_5FenceEEEPKNS_23SurfaceQueueBufferInputEPNS_24SurfaceQueueBufferOutputE",
        ];

        let mut bpf = load_bpf()?;
        let program: &mut UProbe = bpf.program_mut("frame_analyzer_ebpf").unwrap().try_into()?;
        program.load()?;

        let mut last_error = None;
        for symbol in SYMBOLS {
            match program.attach(Some(symbol), 0, LIBGUI, Some(pid)) {
                Ok(_) => {
                    log::info!("attached queueBuffer uprobe for pid {pid} using {symbol}");
                    return Ok(Self { bpf });
                }
                Err(error) => last_error = Some(error),
            }
        }

        let error = last_error.expect("queueBuffer symbol list is non-empty");
        Err(AnalyzerError::AttachError {
            pid,
            attempted: SYMBOLS.join(", "),
            error: error.to_string(),
        })
    }

    pub fn ring(&mut self) -> Result<RingBuf<&mut MapData>> {
        let ring: RingBuf<&mut MapData> = RingBuf::try_from(self.bpf.map_mut("RING_BUF").unwrap())?;
        Ok(ring)
    }

    fn get_program(&mut self) -> Result<&mut UProbe> {
        let program: &mut UProbe = self
            .bpf
            .program_mut("frame_analyzer_ebpf")
            .unwrap()
            .try_into()?;
        Ok(program)
    }
}
