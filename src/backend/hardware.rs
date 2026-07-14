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
                // skip loopback
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
    
    // 1. Dedicated GPU: Nvidia via nvidia-smi
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
    
    // 2. Integrated GPU: AMD or Intel via sysfs
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

pub fn setup_hardware(
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
            // 10 is SysMonitor
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
            let cpu_large_ring_cmds = get_arc_commands(global_cpu / 100.0, 22.5, 25.0, 25.0);
            let ram_large_ring_cmds = get_arc_commands(mem_usage, 22.5, 25.0, 25.0);

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
                    // Reset network base line to prevent spike
                    let (curr_rx, curr_tx) = get_net_bytes();
                    last_rx = curr_rx;
                    last_tx = curr_tx;
                    was_monitor = true;
                }
                
                // Calculate network speeds
                let (curr_rx, curr_tx) = get_net_bytes();
                let rx_speed = if curr_rx >= last_rx { (curr_rx - last_rx) * 2 } else { 0 };
                let tx_speed = if curr_tx >= last_tx { (curr_tx - last_tx) * 2 } else { 0 };
                last_rx = curr_rx;
                last_tx = curr_tx;

                rx_speed_str = format_speed(rx_speed);
                tx_speed_str = format_speed(tx_speed);

                // Calculate GPU stats
                let (dgpu, igpu) = get_gpus_info();
                gpu_name = dgpu.0.clone();
                gpu_usage = dgpu.1;
                igpu_name = igpu.0.clone();
                igpu_usage = igpu.1;

                last_gpu_name = dgpu.0;
                last_gpu_usage = dgpu.1;
                last_igpu_name = igpu.0;
                last_igpu_usage = igpu.1;

                gpu_ring_cmds = get_arc_commands(gpu_usage, 22.5, 25.0, 25.0);
                igpu_ring_cmds = get_arc_commands(igpu_usage, 22.5, 25.0, 25.0);
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

fn parse_wpctl_devices(output: &str, section: &str) -> Vec<crate::AudioDevice> {
    use slint::SharedString;
    let mut devices = Vec::new();
    let mut in_section = false;
    for line in output.lines() {
        if line.contains(section) {
            in_section = true;
            continue;
        }
        if in_section {
            if line.contains("├─") || line.contains("└─") {
                if !line.starts_with(" │  *") && !line.starts_with(" │      ") && !line.starts_with("    *") && !line.starts_with("        ") {
                    break;
                }
            }
            if line.trim().is_empty() { break; }
            
            let is_default = line.contains("*");
            let clean = line.replace("│", "").replace("*", "").trim().to_string();
            if let Some((id_str, rest)) = clean.split_once('.') {
                let id = id_str.trim().to_string();
                let name_part = if let Some((n, _)) = rest.split_once('[') {
                    n.trim().to_string()
                } else {
                    rest.trim().to_string()
                };
                if !id.is_empty() && !name_part.is_empty() && id.chars().all(char::is_numeric) {
                    devices.push(crate::AudioDevice {
                        id: SharedString::from(id),
                        name: SharedString::from(name_part),
                        is_default,
                    });
                }
            }
        }
    }
    devices
}

pub fn setup_mixers(
    ui: &UltimateDynamicIsland,
    active_section: Arc<std::sync::atomic::AtomicU8>,
    notify: Arc<tokio::sync::Notify>,
) {
    use std::sync::atomic::Ordering;
    use slint::{ModelRc, VecModel};
    use std::rc::Rc;
    
    let ui_handle = ui.as_weak();
    let active_section_clone = active_section.clone();
    let notify_clone = notify.clone();
    
    tokio::spawn(async move {
        loop {
            let section_u8 = active_section_clone.load(Ordering::SeqCst);
            // 4 is SysControls
            let is_controls = section_u8 == 4;
            
            if is_controls {
                let mut vol_val = 50.0;
                if let Ok(out) = std::process::Command::new("wpctl").args(&["get-volume", "@DEFAULT_AUDIO_SINK@"]).output() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    if let Some(v_str) = s.split_whitespace().nth(1) {
                        if let Ok(v) = v_str.parse::<f32>() {
                            vol_val = v * 100.0;
                        }
                    }
                }
                
                let mut mic_val = 50.0;
                if let Ok(out) = std::process::Command::new("wpctl").args(&["get-volume", "@DEFAULT_AUDIO_SOURCE@"]).output() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    if let Some(v_str) = s.split_whitespace().nth(1) {
                        if let Ok(v) = v_str.parse::<f32>() {
                            mic_val = v * 100.0;
                        }
                    }
                }
                
                let mut bright_val = 50.0;
                if let Ok(get_out) = std::process::Command::new("brightnessctl").arg("get").output() {
                    if let Ok(max_out) = std::process::Command::new("brightnessctl").arg("max").output() {
                        let get_s = String::from_utf8_lossy(&get_out.stdout);
                        let max_s = String::from_utf8_lossy(&max_out.stdout);
                        if let (Ok(g), Ok(m)) = (get_s.trim().parse::<f32>(), max_s.trim().parse::<f32>()) {
                            if m > 0.0 {
                                bright_val = (g / m) * 100.0;
                            }
                        }
                    }
                }
                
                let mut sinks = Vec::new();
                let mut sources = Vec::new();
                if let Ok(status_out) = std::process::Command::new("wpctl").arg("status").output() {
                    let status_s = String::from_utf8_lossy(&status_out.stdout);
                    sinks = parse_wpctl_devices(&status_s, "Sinks:");
                    sources = parse_wpctl_devices(&status_s, "Sources:");
                }
                
                let _ = slint::invoke_from_event_loop({
                    let ui_handle = ui_handle.clone();
                    move || {
                        let rc_sinks = ModelRc::from(Rc::new(VecModel::from(sinks)));
                        let rc_sources = ModelRc::from(Rc::new(VecModel::from(sources)));
                        if let Some(ui) = ui_handle.upgrade() {
                            ui.set_volume_level(vol_val);
                            ui.set_mic_level(mic_val);
                            ui.set_brightness_level(bright_val);
                            ui.set_audio_sinks(rc_sinks);
                            ui.set_audio_sources(rc_sources);
                        }
                    }
                });
                
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                    _ = notify_clone.notified() => {}
                }
            } else {
                notify_clone.notified().await;
            }
        }
    });
}

pub fn setup_connectivity(
    ui: &UltimateDynamicIsland,
    active_section: Arc<std::sync::atomic::AtomicU8>,
    notify: Arc<tokio::sync::Notify>,
) {
    use std::sync::atomic::Ordering;
    use slint::{SharedString, ModelRc, VecModel};
    use std::rc::Rc;
    use crate::{WifiNetwork, BluetoothDevice};
    
    let ui_handle = ui.as_weak();
    let active_section_clone = active_section.clone();
    let notify_clone = notify.clone();
    
    tokio::spawn(async move {
        loop {
            let section_u8 = active_section_clone.load(Ordering::SeqCst);
            // 4 is SysControls
            let is_controls = section_u8 == 4;
            
            if is_controls {
                let wifi_out = std::process::Command::new("nmcli").arg("-t").arg("-f").arg("SSID,SIGNAL,ACTIVE").arg("dev").arg("wifi").output();
                let mut networks = Vec::new();
                if let Ok(out) = wifi_out {
                    let lines = String::from_utf8_lossy(&out.stdout);
                    for line in lines.lines() {
                        let parts: Vec<&str> = line.split(':').collect();
                        if parts.len() >= 3 && !parts[0].is_empty() {
                            let ssid = parts[0].to_string();
                            let signal = parts[1].parse::<i32>().unwrap_or(0);
                            let connected = parts[2] == "yes";
                            networks.push((ssid, signal, connected));
                        }
                    }
                }
                
                let wifi_enabled_out = std::process::Command::new("nmcli").arg("radio").arg("wifi").output();
                let wifi_enabled = if let Ok(out) = wifi_enabled_out {
                    let output_str = String::from_utf8_lossy(&out.stdout).to_lowercase();
                    output_str.contains("enabled")
                } else {
                    false
                };
                let bt_out = std::process::Command::new("bluetoothctl").arg("show").output();
                let bt_enabled = if let Ok(out) = bt_out {
                    String::from_utf8_lossy(&out.stdout).contains("Powered: yes")
                } else { false };
                
                let bt_devices_out = std::process::Command::new("bluetoothctl").arg("devices").output();
                let mut bt_devices = Vec::new();
                if let Ok(out) = bt_devices_out {
                    let lines = String::from_utf8_lossy(&out.stdout);
                    for line in lines.lines() {
                        let parts: Vec<&str> = line.splitn(3, ' ').collect();
                        if parts.len() == 3 && parts[0] == "Device" {
                            let mac = parts[1].to_string();
                            let name = parts[2].to_string();
                            
                            let info_out = std::process::Command::new("bluetoothctl").arg("info").arg(&mac).output();
                            let is_connected = if let Ok(info) = info_out {
                                String::from_utf8_lossy(&info.stdout).contains("Connected: yes")
                            } else { false };
                            
                            bt_devices.push((name, mac, is_connected));
                        }
                    }
                }
                
                let eth_out = std::process::Command::new("nmcli").arg("-t").arg("-f").arg("DEVICE,TYPE,STATE").arg("dev").output();
                let mut eth_name = String::new();
                if let Ok(out) = eth_out {
                    for line in String::from_utf8_lossy(&out.stdout).lines() {
                        let parts: Vec<&str> = line.split(':').collect();
                        if parts.len() == 3 && parts[1] == "ethernet" && parts[2] == "connected" {
                            eth_name = parts[0].to_string();
                            break;
                        }
                    }
                }
                
                let _ = slint::invoke_from_event_loop({
                    let ui_handle = ui_handle.clone();
                    move || {
                        if let Some(ui) = ui_handle.upgrade() {
                            let wifi_models: Vec<WifiNetwork> = networks.iter().map(|(ssid, signal, connected)| WifiNetwork {
                                ssid: SharedString::from(ssid),
                                signal: *signal,
                                connected: *connected,
                            }).collect();
                            ui.set_wifi_networks(ModelRc::from(Rc::new(VecModel::from(wifi_models))));
                            
                            let bt_models: Vec<BluetoothDevice> = bt_devices.iter().map(|(name, mac, connected)| BluetoothDevice {
                                name: SharedString::from(name),
                                mac: SharedString::from(mac),
                                connected: *connected,
                            }).collect();
                            ui.set_bluetooth_devices(ModelRc::from(Rc::new(VecModel::from(bt_models))));
                            
                            ui.set_wifi_enabled(wifi_enabled);
                            ui.set_bluetooth_enabled(bt_enabled);
                            ui.set_ethernet_name(SharedString::from(&eth_name));
                        }
                    }
                });
                
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                    _ = notify_clone.notified() => {}
                }
            } else {
                notify_clone.notified().await;
            }
        }
    });
}
