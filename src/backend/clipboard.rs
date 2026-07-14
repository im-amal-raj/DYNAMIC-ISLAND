use slint::{SharedString, ComponentHandle, Image, SharedPixelBuffer, Rgba8Pixel, ModelRc, VecModel};
use std::time::Duration;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use crate::{UltimateDynamicIsland, IslandSection, ClipItem};

pub fn setup_clipboard(ui: &UltimateDynamicIsland) {
    let search_query = Arc::new(Mutex::new(String::new()));
    let search_notify = Arc::new(tokio::sync::Notify::new());
    
    let paste_ui_handle = ui.as_weak();
    ui.on_paste_clip(move |id| {
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("cliphist decode {} | wl-copy", id))
            .spawn();
        if let Some(ui) = paste_ui_handle.upgrade() {
            ui.set_active_section(IslandSection::Compact);
            ui.set_target_width(220.0);
            ui.set_target_height(42.0);
        }
    });

    let clear_ui_handle = ui.as_weak();
    let clear_notify = search_notify.clone();
    ui.on_clear_clipboard(move || {
        let _ = std::process::Command::new("cliphist").arg("wipe").spawn();
        if let Some(ui) = clear_ui_handle.upgrade() {
            ui.set_clipboard_history(ModelRc::default());
        }
        clear_notify.notify_one();
    });

    let search_query_setter = search_query.clone();
    let search_notify_setter = search_notify.clone();
    ui.on_search_clipboard(move |q| {
        if let Ok(mut g) = search_query_setter.lock() {
            *g = q.to_string();
        }
        search_notify_setter.notify_one();
    });

    let erase_notify_setter = search_notify.clone();
    ui.on_erase_clip(move |id| {
        let cmd = format!("cliphist list | grep -E '^{}\t' | cliphist delete", id);
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .status();
        erase_notify_setter.notify_one();
    });

    let clip_ui_handle = ui.as_weak();
    let query_state = search_query.clone();
    let notify_loop = search_notify.clone();
    tokio::spawn(async move {
        let mut last_stdout = String::new();
        let mut last_query = String::new();
        loop {
            // Await trigger or wake up every 1 second
            let _ = tokio::time::timeout(Duration::from_secs(1), notify_loop.notified()).await;
            
            let query = {
                if let Ok(q) = query_state.lock() {
                    q.clone()
                } else {
                    String::new()
                }
            };
            
            let cmd = if query.is_empty() {
                "cliphist list | head -n 4".to_string()
            } else {
                "cliphist list | head -n 50".to_string()
            };
            
            if let Ok(output) = std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output() 
            {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                if stdout == last_stdout && query == last_query {
                    continue;
                }
                last_stdout = stdout.clone();
                last_query = query.clone();
                
                let lines: Vec<&str> = stdout.trim().split('\n').filter(|l| !l.is_empty()).collect();
                
                let mut raw_items = Vec::new();
                let query_lower = query.to_lowercase();
                for line in lines {
                    let parts: Vec<&str> = line.splitn(2, '\t').collect();
                    if parts.len() == 2 {
                        let id = parts[0];
                        let content = parts[1];
                        
                        let has_image = content.starts_with("[[ binary data");
                        let text = if has_image { "Image Snippet".to_string() } else { content.to_string() };
                        
                        if !query_lower.is_empty() && !text.to_lowercase().contains(&query_lower) {
                            continue;
                        }
                        
                        let mut image_data = None;
                        
                        if has_image {
                            if let Ok(img_output) = std::process::Command::new("sh")
                                .arg("-c")
                                .arg(format!("cliphist decode {}", id))
                                .output() {
                                if let Ok(img) = image::load_from_memory(&img_output.stdout) {
                                    let rgba = img.into_rgba8();
                                    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                                        rgba.as_raw(),
                                        rgba.width(),
                                        rgba.height(),
                                    );
                                    image_data = Some(buffer);
                                }
                            }
                        }
                        
                        raw_items.push((
                            id.to_string(),
                            text,
                            has_image,
                            image_data,
                        ));
                    }
                }
                
                // Limit output to top 4 matches for display space
                let final_len = std::cmp::min(raw_items.len(), 4);
                raw_items.truncate(final_len);
                
                let _ = slint::invoke_from_event_loop({
                    let ui_handle = clip_ui_handle.clone();
                    move || {
                        if let Some(ui) = ui_handle.upgrade() {
                            let mut slint_items = Vec::new();
                            for (id, text, has_image, img_data) in raw_items {
                                slint_items.push(ClipItem {
                                    id: SharedString::from(id),
                                    text: SharedString::from(text),
                                    has_image,
                                    image_data: img_data.map(|b| Image::from_rgba8(b)).unwrap_or_default(),
                                });
                            }
                            let model = Rc::new(VecModel::from(slint_items));
                            ui.set_clipboard_history(ModelRc::from(model));
                        }
                    }
                });
            }
        }
    });
}
