use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use slint::{ModelRc, VecModel, SharedString, ComponentHandle};
use std::rc::Rc;
use crate::WifiNetwork;

use crate::UltimateDynamicIsland;

pub fn setup(
    ui: &UltimateDynamicIsland,
    active_section: Arc<AtomicU8>,
    notify: Arc<Notify>,
) {
    let ui_handle = ui.as_weak();
    let active_section_clone = active_section.clone();
    let notify_clone = notify.clone();
    
    tokio::spawn(async move {
        let mut is_first_run = true;
        loop {
            let section_u8 = active_section_clone.load(Ordering::SeqCst);
            // 4 is SysControls
            let is_controls = section_u8 == 4;
            
            if is_controls || is_first_run {
                is_first_run = false;
                let wifi_out = std::process::Command::new("nmcli")
                    .args(&["--rescan", "no", "-t", "-f", "SSID,SIGNAL,ACTIVE", "dev", "wifi"])
                    .output();
                let mut networks = Vec::new();
                if let Ok(out) = wifi_out {
                    let lines = String::from_utf8_lossy(&out.stdout);
                    for line in lines.lines() {
                        let parts: Vec<&str> = line.rsplitn(3, ':').collect();
                        if parts.len() >= 3 {
                            let connected = parts[0] == "yes";
                            let signal = parts[1].parse::<i32>().unwrap_or(0);
                            // rsplitn reverses the order, so parts[2] is the SSID (everything before the last two colons)
                            let ssid = parts[2].replace("\\:", ":"); // Unescape colons
                            if !ssid.is_empty() {
                                networks.push((ssid.to_string(), signal, connected));
                            }
                        }
                    }
                }
                
                // Fallback to iw if nmcli failed or returned nothing
                if networks.is_empty() {
                    if let Ok(out) = std::process::Command::new("iw").arg("dev").output() {
                        let out_str = String::from_utf8_lossy(&out.stdout);
                        if let Some(iface) = out_str.lines().find(|l| l.trim().starts_with("Interface")).map(|l| l.replace("Interface ", "").trim().to_string()) {
                            if let Ok(link_out) = std::process::Command::new("iw").arg("dev").arg(&iface).arg("link").output() {
                                let link_str = String::from_utf8_lossy(&link_out.stdout);
                                if link_str.contains("Connected to") || link_str.contains("SSID:") {
                                    if let Some(ssid_line) = link_str.lines().find(|l| l.trim().starts_with("SSID:")) {
                                        let ssid = ssid_line.trim().replace("SSID: ", "");
                                        networks.push((ssid.to_string(), 85, true)); // Dummy signal
                                    }
                                }
                            }
                        }
                    }
                }
                
                let wifi_enabled_out = std::process::Command::new("nmcli").arg("radio").arg("wifi").output();
                let mut wifi_enabled = if let Ok(out) = wifi_enabled_out {
                    let output_str = String::from_utf8_lossy(&out.stdout).to_lowercase();
                    output_str.contains("enabled")
                } else {
                    false
                };
                
                if !wifi_enabled {
                    if let Ok(out) = std::process::Command::new("rfkill").arg("list").arg("wifi").output() {
                        let out_str = String::from_utf8_lossy(&out.stdout);
                        if out_str.contains("Soft blocked: no") && out_str.contains("Hard blocked: no") {
                            wifi_enabled = true;
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
                            ui.set_wifi_enabled(wifi_enabled);
                            ui.set_ethernet_name(SharedString::from(&eth_name));
                        }
                    }
                });
                
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(1500)) => {}
                    _ = notify_clone.notified() => {}
                }
            } else {
                notify_clone.notified().await;
            }
        }
    });
}
