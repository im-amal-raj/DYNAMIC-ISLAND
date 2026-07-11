use slint::{SharedString, ComponentHandle};
use std::time::Duration;
use sysinfo::{System, CpuRefreshKind, RefreshKind, MemoryRefreshKind};
use crate::UltimateDynamicIsland;

fn get_net_bytes() -> (u64, u64) {
    let mut rx_bytes = 0;
    let mut tx_bytes = 0;
    if let Ok(contents) = std::fs::read_to_string("/proc/net/dev") {
        for line in contents.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() > 8 && !parts[0].starts_with("lo") {
                if let Ok(rx) = parts[1].parse::<u64>() {
                    rx_bytes += rx;
                }
                if let Ok(tx) = parts[9].parse::<u64>() {
                    tx_bytes += tx;
                }
            }
        }
    }
    (rx_bytes, tx_bytes)
}

fn format_speed(bytes_per_sec: u64) -> String {
    if bytes_per_sec < 1024 {
        format!("{} B/s", bytes_per_sec)
    } else if bytes_per_sec < 1024 * 1024 {
        format!("{:.1} KB/s", bytes_per_sec as f64 / 1024.0)
    } else {
        format!("{:.1} MB/s", bytes_per_sec as f64 / (1024.0 * 1024.0))
    }
}

fn get_gpu_info() -> (String, f32) {
    // 1. Try Nvidia (nvidia-smi)
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args(&["--query-gpu=utilization.gpu,name", "--format=csv,noheader,nounits"])
        .output()
    {
        let out_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = out_str.split(',').collect();
        if parts.len() >= 2 {
            let usage = parts[0].trim().parse::<f32>().unwrap_or(0.0) / 100.0;
            let name = parts[1].trim().to_string();
            return (name, usage);
        }
    }

    // 2. Try AMD/Intel GPU via sysfs
    for card in &["card1", "card0"] {
        let path_busy = format!("/sys/class/drm/{}/device/gpu_busy_percent", card);
        if let Ok(busy_str) = std::fs::read_to_string(&path_busy) {
            if let Ok(busy) = busy_str.trim().parse::<f32>() {
                let path_vendor = format!("/sys/class/drm/{}/device/vendor", card);
                let vendor = std::fs::read_to_string(&path_vendor).unwrap_or_default().trim().to_string();
                let name = if vendor.contains("0x1002") {
                    "AMD Radeon GPU".to_string()
                } else if vendor.contains("0x8086") {
                    "Intel UHD/Iris GPU".to_string()
                } else {
                    "Integrated GPU".to_string()
                };
                return (name, busy / 100.0);
            }
        }
    }

    ("Intel/AMD Graphics".to_string(), 0.0)
}

pub fn setup_hardware(ui: &UltimateDynamicIsland) {
    let cpu_ui_handle = ui.as_weak();
    tokio::spawn(async move {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );
        
        let (mut last_rx, mut last_tx) = get_net_bytes();
        
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            sys.refresh_all();
            let global_cpu = sys.global_cpu_info().cpu_usage();
            let used_mem = sys.used_memory();
            let total_mem = sys.total_memory();
            let mem_usage = if total_mem > 0 { used_mem as f32 / total_mem as f32 } else { 0.0 };

            // Calculate network speeds
            let (curr_rx, curr_tx) = get_net_bytes();
            let rx_speed = if curr_rx >= last_rx { (curr_rx - last_rx) * 2 } else { 0 };
            let tx_speed = if curr_tx >= last_tx { (curr_tx - last_tx) * 2 } else { 0 };
            last_rx = curr_rx;
            last_tx = curr_tx;

            let rx_speed_str = format_speed(rx_speed);
            let tx_speed_str = format_speed(tx_speed);

            // Calculate GPU stats
            let (gpu_name, gpu_usage) = get_gpu_info();

            let _ = slint::invoke_from_event_loop({
                let ui_handle = cpu_ui_handle.clone();
                move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_cpu_usage(global_cpu / 100.0);
                        ui.set_memory_usage(mem_usage);
                        ui.set_net_down_speed(SharedString::from(rx_speed_str));
                        ui.set_net_up_speed(SharedString::from(tx_speed_str));
                        ui.set_gpu_name(SharedString::from(gpu_name));
                        ui.set_gpu_usage(gpu_usage);
                    }
                }
            });
        }
    });
}
