use sysinfo::{System, RefreshKind, CpuRefreshKind, MemoryRefreshKind};
use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::Notify;
use slint::{SharedString, ComponentHandle};

use crate::UltimateDynamicIsland;

fn get_net_bytes() -> (u64, u64) {
    if let Ok(data) = std::fs::read_to_string("/proc/net/dev") {
        let mut total_rx = 0;
        let mut total_tx = 0;
        for line in data.lines().skip(2) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 9 {
                if parts[0].starts_with("lo") {
                    continue;
                }
                let rx: u64 = parts[1].parse().unwrap_or(0);
                let tx: u64 = parts[9].parse().unwrap_or(0);
                total_rx += rx;
                total_tx += tx;
            }
        }
        return (total_rx, total_tx);
    }
    (0, 0)
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

fn get_gpus_info() -> ((String, f32), (String, f32)) {
    let mut dgpu = ("Nvidia Graphics".to_string(), 0.0);
    let mut igpu = ("AMD/Intel Graphics".to_string(), 0.0);
    
    if let Ok(output) = std::process::Command::new("nvidia-smi")
        .args(&["--query-gpu=utilization.gpu,name", "--format=csv,noheader,nounits"])
        .output()
    {
        let out_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = out_str.split(',').collect();
        if parts.len() >= 2 {
            let usage = parts[0].trim().parse::<f32>().unwrap_or(0.0) / 100.0;
            let name = parts[1].trim().to_string();
            dgpu = (name, usage);
        }
    }
    
    for card in &["card0", "card1", "card2"] {
        let path_vendor = format!("/sys/class/drm/{}/device/vendor", card);
        if let Ok(vendor_raw) = std::fs::read_to_string(&path_vendor) {
            let vendor = vendor_raw.trim().to_string();
            let path_busy = format!("/sys/class/drm/{}/device/gpu_busy_percent", card);
            if let Ok(busy_str) = std::fs::read_to_string(&path_busy) {
                if let Ok(busy) = busy_str.trim().parse::<f32>() {
                    let name = if vendor.contains("0x1002") {
                        "AMD Radeon GPU".to_string()
                    } else if vendor.contains("0x8086") {
                        "Intel UHD/Iris GPU".to_string()
                    } else {
                        "Integrated GPU".to_string()
                    };
                    igpu = (name, busy / 100.0);
                    break;
                }
            }
        }
    }
    
    (dgpu, igpu)
}

fn get_arc_commands(percentage: f32, radius: f32, cx: f32, cy: f32) -> String {
    let p = percentage.clamp(0.0, 0.9999);
    if p < 0.001 {
        return String::new();
    }
    let angle = p * 2.0 * std::f32::consts::PI;
    let start_x = cx;
    let start_y = cy - radius;
    let end_x = cx + radius * angle.sin();
    let end_y = cy - radius * angle.cos();
    let large_arc = if p > 0.5 { 1 } else { 0 };
    format!("M {} {} A {} {} 0 {} 1 {} {}", start_x, start_y, radius, radius, large_arc, end_x, end_y)
}

pub fn setup(
    ui: &UltimateDynamicIsland,
    active_section: Arc<AtomicU8>,
    notify: Arc<Notify>,
) {
    let cpu_ui_handle = ui.as_weak();
    tokio::spawn(async move {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );
        
        let (mut last_rx, mut last_tx) = get_net_bytes();
        let mut last_gpu_name = String::from("Nvidia Graphics");
        let mut last_gpu_usage = 0.0;
        let mut last_igpu_name = String::from("AMD/Intel Graphics");
        let mut last_igpu_usage = 0.0;
        let mut was_monitor = false;
        
        loop {
            let section_u8 = active_section.load(Ordering::SeqCst);
            let is_monitor = section_u8 == 10;
            
            if is_monitor {
                sys.refresh_all();
            } else {
                sys.refresh_all();
                sys.refresh_memory();
            }
            
            let global_cpu = sys.global_cpu_info().cpu_usage();
            let used_mem = sys.used_memory();
            let total_mem = sys.total_memory();
            let mem_usage = if total_mem > 0 { used_mem as f32 / total_mem as f32 } else { 0.0 };

            let cpu_ring_cmds = get_arc_commands(global_cpu / 100.0, 11.0, 12.0, 12.0);
            let ram_ring_cmds = get_arc_commands(mem_usage, 8.5, 12.0, 12.0);
            let cpu_large_ring_cmds = get_arc_commands(global_cpu / 100.0, 27.0, 30.0, 30.0);
            let ram_large_ring_cmds = get_arc_commands(mem_usage, 27.0, 30.0, 30.0);

            let mut rx_speed_str = String::from("0.0 KB/s");
            let mut tx_speed_str = String::from("0.0 KB/s");
            let mut gpu_name = last_gpu_name.clone();
            let mut gpu_usage = last_gpu_usage;
            let mut igpu_name = last_igpu_name.clone();
            let mut igpu_usage = last_igpu_usage;
            let mut gpu_ring_cmds = String::new();
            let mut igpu_ring_cmds = String::new();

            if is_monitor {
                if !was_monitor {
                    let (curr_rx, curr_tx) = get_net_bytes();
                    last_rx = curr_rx;
                    last_tx = curr_tx;
                    was_monitor = true;
                }
                
                let (curr_rx, curr_tx) = get_net_bytes();
                let rx_speed = if curr_rx >= last_rx { (curr_rx - last_rx) * 2 } else { 0 };
                let tx_speed = if curr_tx >= last_tx { (curr_tx - last_tx) * 2 } else { 0 };
                last_rx = curr_rx;
                last_tx = curr_tx;

                rx_speed_str = format_speed(rx_speed);
                tx_speed_str = format_speed(tx_speed);

                let (dgpu, igpu) = get_gpus_info();
                gpu_name = dgpu.0.clone();
                gpu_usage = dgpu.1;
                igpu_name = igpu.0.clone();
                igpu_usage = igpu.1;

                last_gpu_name = dgpu.0;
                last_gpu_usage = dgpu.1;
                last_igpu_name = igpu.0;
                last_igpu_usage = igpu.1;

                gpu_ring_cmds = get_arc_commands(gpu_usage, 27.0, 30.0, 30.0);
                igpu_ring_cmds = get_arc_commands(igpu_usage, 27.0, 30.0, 30.0);
            } else {
                was_monitor = false;
            }

            let _ = slint::invoke_from_event_loop({
                let ui_handle = cpu_ui_handle.clone();
                let rx_speed_str_clone = rx_speed_str.clone();
                let tx_speed_str_clone = tx_speed_str.clone();
                let gpu_name_clone = gpu_name.clone();
                let igpu_name_clone = igpu_name.clone();
                let cpu_ring_cmds_clone = cpu_ring_cmds.clone();
                let ram_ring_cmds_clone = ram_ring_cmds.clone();
                let cpu_large_ring_cmds_clone = cpu_large_ring_cmds.clone();
                let ram_large_ring_cmds_clone = ram_large_ring_cmds.clone();
                let gpu_ring_cmds_clone = gpu_ring_cmds.clone();
                let igpu_ring_cmds_clone = igpu_ring_cmds.clone();
                move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_cpu_usage(global_cpu / 100.0);
                        ui.set_memory_usage(mem_usage);
                        ui.set_cpu_ring_commands(SharedString::from(cpu_ring_cmds_clone));
                        ui.set_ram_ring_commands(SharedString::from(ram_ring_cmds_clone));
                        ui.set_cpu_large_ring_commands(SharedString::from(cpu_large_ring_cmds_clone));
                        ui.set_ram_large_ring_commands(SharedString::from(ram_large_ring_cmds_clone));
                        if is_monitor {
                            ui.set_net_down_speed(SharedString::from(rx_speed_str_clone));
                            ui.set_net_up_speed(SharedString::from(tx_speed_str_clone));
                            ui.set_gpu_name(SharedString::from(gpu_name_clone));
                            ui.set_gpu_usage(gpu_usage);
                            ui.set_gpu_ring_commands(SharedString::from(gpu_ring_cmds_clone));
                            ui.set_igpu_name(SharedString::from(igpu_name_clone));
                            ui.set_igpu_usage(igpu_usage);
                            ui.set_igpu_ring_commands(SharedString::from(igpu_ring_cmds_clone));
                        }
                    }
                }
            });
            
            let sleep_dur = if is_monitor {
                Duration::from_millis(500)
            } else {
                Duration::from_millis(2000)
            };
            
            tokio::select! {
                _ = tokio::time::sleep(sleep_dur) => {}
                _ = notify.notified() => {}
            }
        }
    });
}
