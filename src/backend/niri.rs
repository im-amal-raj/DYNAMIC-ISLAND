
use crate::{UltimateDynamicIsland, IslandSection};

pub fn setup(ui_handle: slint::Weak<UltimateDynamicIsland>) {
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        if let Ok(mut child) = tokio::process::Command::new("niri")
            .args(&["msg", "-j", "event-stream"])
            .stdout(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(stdout) = child.stdout.take() {
                let mut reader = tokio::io::BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    if line.contains("WindowFocusChanged") {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&line) {
                            if let Some(focus_changed) = parsed.get("WindowFocusChanged") {
                                // If focus_changed is null, it means no window has focus
                                if focus_changed.is_null() {
                                    let _ = slint::invoke_from_event_loop({
                                        let ui = ui_handle.clone();
                                        move || {
                                            if let Some(ui) = ui.upgrade() {
                                                if ui.get_active_section() != IslandSection::Compact && ui.get_active_section() != IslandSection::Media {
                                                    ui.set_active_section(IslandSection::Compact);
                                                }
                                            }
                                        }
                                    });
                                } else if let Some(_id) = focus_changed.get("id") {
                                    // A specific window was focused
                                    let mut is_our_window = false;
                                    if let Some(app_id) = focus_changed.get("app_id").and_then(|a| a.as_str()) {
                                        if app_id.contains("rust-island") || app_id.contains("dynamic-island") {
                                            is_our_window = true;
                                        }
                                    }
                                    if let Some(title) = focus_changed.get("title").and_then(|t| t.as_str()) {
                                        if title.contains("rust-island") || title.contains("dynamic-island") {
                                            is_our_window = true;
                                        }
                                    }
                                    
                                    if !is_our_window {
                                        let _ = slint::invoke_from_event_loop({
                                            let ui = ui_handle.clone();
                                            move || {
                                                if let Some(ui) = ui.upgrade() {
                                                    if ui.get_active_section() != IslandSection::Compact && ui.get_active_section() != IslandSection::Media {
                                                        ui.set_active_section(IslandSection::Compact);
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}
