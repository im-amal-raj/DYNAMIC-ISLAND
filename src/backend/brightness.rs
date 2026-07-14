use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use slint::ComponentHandle;

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
        loop {
            let section_u8 = active_section_clone.load(Ordering::SeqCst);
            // 4 is SysControls
            let is_controls = section_u8 == 4;
            
            if is_controls {
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
                
                let _ = slint::invoke_from_event_loop({
                    let ui_handle = ui_handle.clone();
                    move || {
                        if let Some(ui) = ui_handle.upgrade() {
                            ui.set_brightness_level(bright_val);
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
