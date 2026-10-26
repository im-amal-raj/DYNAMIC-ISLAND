use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use slint::{ModelRc, VecModel, SharedString, ComponentHandle};
use std::rc::Rc;
use crate::BluetoothDevice;

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
                let bt_out = std::process::Command::new("bluetoothctl").arg("show").output();
                let bt_enabled = if let Ok(out) = bt_out {
                    String::from_utf8_lossy(&out.stdout).contains("Powered: yes")
                } else { false };
                
                let connected_set: std::collections::HashSet<String> = if let Ok(out) = std::process::Command::new("bluetoothctl").args(&["devices", "Connected"]).output() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    s.lines().filter_map(|l| {
                        let parts: Vec<&str> = l.splitn(3, ' ').collect();
                        if parts.len() >= 2 && parts[0] == "Device" {
                            Some(parts[1].to_string())
                        } else { None }
                    }).collect()
                } else {
                    std::collections::HashSet::new()
                };
                
                let bt_devices_out = std::process::Command::new("bluetoothctl").arg("devices").output();
                let mut bt_devices = Vec::new();
                if let Ok(out) = bt_devices_out {
                    let lines = String::from_utf8_lossy(&out.stdout);
                    for line in lines.lines() {
                        let parts: Vec<&str> = line.splitn(3, ' ').collect();
                        if parts.len() == 3 && parts[0] == "Device" {
                            let mac = parts[1].to_string();
                            let name = parts[2].to_string();
                            let is_connected = connected_set.contains(&mac);
                            bt_devices.push((name, mac, is_connected));
                        }
                    }
                }
                
                let _ = slint::invoke_from_event_loop({
                    let ui_handle = ui_handle.clone();
                    move || {
                        if let Some(ui) = ui_handle.upgrade() {
                            let bt_models: Vec<BluetoothDevice> = bt_devices.iter().map(|(name, mac, connected)| BluetoothDevice {
                                name: SharedString::from(name),
                                mac: SharedString::from(mac),
                                connected: *connected,
                            }).collect();
                            ui.set_bluetooth_devices(ModelRc::from(Rc::new(VecModel::from(bt_models))));
                            ui.set_bluetooth_enabled(bt_enabled);
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
