use std::{fs::OpenOptions, io::Write};

use eframe::egui_wgpu::RenderState;

pub fn write(render_state: Option<&RenderState>) {
    let (working_set, private) = process_memory();
    let (buffers, textures) = match render_state {
        Some(state) => {
            let counters = state.device.get_internal_counters();
            (counters.hal.buffer_memory.read(), counters.hal.texture_memory.read())
        }
        None => (0, 0),
    };

    let line = format!(
        "ws_mb={:.1} private_mb={:.1} wgpu_buffer_mb={:.1} wgpu_texture_mb={:.1}\n",
        working_set as f64 / 1_048_576.0,
        private as f64 / 1_048_576.0,
        buffers as f64 / 1_048_576.0,
        textures as f64 / 1_048_576.0,
    );
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("status-panel-mem.log")
    {
        let _ = file.write_all(line.as_bytes());
    }
}

#[cfg(windows)]
fn process_memory() -> (u64, u64) {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    let ok = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters as *mut _ as *mut _,
            counters.cb,
        )
    };
    if ok == 0 {
        (0, 0)
    } else {
        (counters.WorkingSetSize as u64, counters.PrivateUsage as u64)
    }
}

#[cfg(not(windows))]
fn process_memory() -> (u64, u64) {
    let Ok(status) = std::fs::read_to_string("/proc/self/statm") else {
        return (0, 0);
    };
    let pages = status
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    (pages * 4096, 0)
}
