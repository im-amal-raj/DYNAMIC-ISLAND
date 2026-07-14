use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use slint::{ModelRc, VecModel, SharedString, ComponentHandle};
use std::rc::Rc;

use crate::UltimateDynamicIsland;

fn parse_wpctl_devices(output: &str, section: &str) -> Vec<crate::AudioDevice> {
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

pub fn setup(
    ui: &UltimateDynamicIsland,
    active_section: Arc<AtomicU8>,
    notify: Arc<Notify>,
) {
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
