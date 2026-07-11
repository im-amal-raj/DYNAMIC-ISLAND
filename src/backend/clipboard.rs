use slint::{SharedString, ComponentHandle, Image, SharedPixelBuffer, Rgba8Pixel, ModelRc, VecModel};
use std::time::Duration;
use std::rc::Rc;
use crate::{UltimateDynamicIsland, IslandSection, ClipItem};

pub fn setup_clipboard(ui: &UltimateDynamicIsland) {
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
    ui.on_clear_clipboard(move || {
        let _ = std::process::Command::new("cliphist").arg("wipe").spawn();
        if let Some(ui) = clear_ui_handle.upgrade() {
            ui.set_clipboard_history(ModelRc::default());
        }
    });

    let clip_ui_handle = ui.as_weak();
    tokio::spawn(async move {
        let mut last_stdout = String::new();
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            if let Ok(output) = std::process::Command::new("sh")
                .arg("-c")
                .arg("cliphist list | head -n 4")
                .output() 
            {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                if stdout == last_stdout {
                    continue;
                }
                last_stdout = stdout.clone();
                
                let lines: Vec<&str> = stdout.trim().split('\n').filter(|l| !l.is_empty()).collect();
                
                let mut raw_items = Vec::new();
                for line in lines {
                    let parts: Vec<&str> = line.splitn(2, '\t').collect();
                    if parts.len() == 2 {
                        let id = parts[0];
                        let content = parts[1];
                        
                        let has_image = content.starts_with("[[ binary data");
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
                            if has_image { "Image Snippet".to_string() } else { content.to_string() },
                            has_image,
                            image_data,
                        ));
                    }
                }
                
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
