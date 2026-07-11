use slint::{SharedString, ComponentHandle};
use std::time::Duration;
use crate::UltimateDynamicIsland;
use log::{info, warn};

pub fn setup_media(ui: &UltimateDynamicIsland) {
    ui.on_media_action(|action| {
        let arg = match action.as_str() {
            "previous" => "previous",
            "play-pause" => "play-pause",
            "next" => "next",
            _ => "",
        };
        if !arg.is_empty() {
            let _ = std::process::Command::new("playerctl").arg(arg).spawn();
        }
    });

    let mpris_ui_handle = ui.as_weak();
    tokio::spawn(async move {
        let mut last_art_url = String::new();
        
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            let mut extracted = None;
            let mut play_info = None;
            {
                if let Ok(finder) = mpris::PlayerFinder::new() {
                    if let Ok(players) = finder.find_all() {
                        if let Some(player) = players.into_iter().next() {
                            if let Ok(metadata) = player.get_metadata() {
                                let title = metadata.title().unwrap_or("Unknown").to_string();
                                let artist = metadata.artists().map(|a| a.join(", ")).unwrap_or_else(|| "Unknown".to_string());
                                let art_url = metadata.art_url().unwrap_or("").to_string();
                                let raw_url = metadata.url().unwrap_or("").to_string();
                                let is_playing = player.get_playback_status().unwrap_or(mpris::PlaybackStatus::Stopped) == mpris::PlaybackStatus::Playing;
                                let bus_name = player.bus_name().to_string();
                                play_info = Some((bus_name, title, artist, art_url, raw_url, is_playing));
                            }
                        }
                    }
                }
            }

            if let Some((bus_name, title, artist, mut art_url, raw_url, is_playing)) = play_info {
                crate::backend::log_to_file(&format!(
                    "Media Loop: player={}, title='{}', artist='{}', raw_art_url='{}', raw_url='{}'",
                    bus_name, title, artist, art_url, raw_url
                ));
                
                let is_browser = bus_name.contains("chrome") || 
                                 bus_name.contains("chromium") || 
                                 bus_name.contains("brave") || 
                                 bus_name.contains("firefox");
                let is_youtube = title.ends_with(" - YouTube") || raw_url.contains("youtube.com") || raw_url.contains("youtu.be");
                
                if is_browser && is_youtube {
                    crate::backend::log_to_file("Media Loop: YouTube playing in browser. Resolving actual YouTube thumbnail...");
                    let mut yt_id = None;
                    if !raw_url.is_empty() {
                        yt_id = extract_youtube_id(&raw_url);
                    }
                    if yt_id.is_none() {
                        yt_id = search_youtube_for_id(&title).await;
                    }
                    if let Some(id) = yt_id {
                        let yt_art = format!("https://img.youtube.com/vi/{}/hqdefault.jpg", id);
                        crate::backend::log_to_file(&format!("Media Loop: Overriding art_url with actual YouTube thumbnail: {}", yt_art));
                        art_url = yt_art;
                    } else {
                        crate::backend::log_to_file("Media Loop: YouTube ID resolution failed, using default art_url");
                    }
                } else if art_url.is_empty() {
                    if !raw_url.is_empty() {
                        if let Some(yt_id) = extract_youtube_id(&raw_url) {
                            art_url = format!("https://img.youtube.com/vi/{}/hqdefault.jpg", yt_id);
                            crate::backend::log_to_file(&format!(
                                "Media Loop: Extracted YouTube ID='{}', constructed art_url='{}'",
                                yt_id, art_url
                            ));
                        } else {
                            crate::backend::log_to_file(&format!(
                                "Media Loop: Could not extract YouTube ID from url='{}'",
                                raw_url
                            ));
                        }
                    } else {
                        crate::backend::log_to_file("Media Loop: art_url and raw_url are both empty");
                    }
                }
                
                info!("MPRIS found {}: title='{}', art_url='{}'", bus_name, title, art_url);
                extracted = Some((title, artist, art_url, is_playing));
            }


            if let Some((title, artist, art_url, is_playing)) = extracted {
                let mut loaded_image = None;
                let url_changed = art_url != last_art_url;
                
                if url_changed && !art_url.is_empty() {
                    crate::backend::log_to_file(&format!("Media Loop: URL changed. Fetching art: {}", art_url));
                    if art_url.starts_with("file://") {
                        if let Ok(u) = url::Url::parse(&art_url) {
                            if let Ok(path) = u.to_file_path() {
                                crate::backend::log_to_file(&format!("Media Loop: Reading local file: {:?}", path));
                                let mut file_loaded = false;
                                if let Ok(file) = std::fs::File::open(&path) {
                                    let reader = std::io::BufReader::new(file);
                                    if let Ok(img_reader) = image::io::Reader::new(reader).with_guessed_format() {
                                        if let Ok(img) = img_reader.decode() {
                                            loaded_image = Some(img.to_rgba8());
                                            last_art_url = art_url.clone();
                                            crate::backend::log_to_file("Media Loop: Successfully loaded local image via guess_format");
                                            file_loaded = true;
                                        } else {
                                            crate::backend::log_to_file("Media Loop: Failed to decode guessed image content");
                                        }
                                    } else {
                                        crate::backend::log_to_file("Media Loop: Failed to guess format of local image file");
                                    }
                                } else {
                                    crate::backend::log_to_file(&format!("Media Loop: Failed to open local image file: {:?}", path));
                                }

                                if !file_loaded {
                                    if let Ok(img) = image::open(&path) {
                                        loaded_image = Some(img.to_rgba8());
                                        last_art_url = art_url.clone();
                                        crate::backend::log_to_file("Media Loop: Successfully loaded local image via direct open fallback");
                                    } else {
                                        crate::backend::log_to_file(&format!("Media Loop: Direct open fallback also failed for {:?}", path));
                                        warn!("Failed to parse local image: {:?}", path);
                                    }
                                }
                            }
                        }
                    } else if art_url.starts_with("http") {
                        crate::backend::log_to_file(&format!("Media Loop: Fetching remote image: {}", art_url));
                        match reqwest::get(&art_url).await {
                            Ok(resp) => {
                                crate::backend::log_to_file(&format!("Media Loop: HTTP response status={}", resp.status()));
                                match resp.bytes().await {
                                    Ok(bytes) => {
                                        crate::backend::log_to_file(&format!("Media Loop: Downloaded {} bytes", bytes.len()));
                                        match image::load_from_memory(&bytes) {
                                            Ok(img) => {
                                                crate::backend::log_to_file("Media Loop: Successfully decoded image bytes");
                                                loaded_image = Some(img.to_rgba8());
                                                last_art_url = art_url.clone();
                                            }
                                            Err(e) => {
                                                crate::backend::log_to_file(&format!("Media Loop: Failed to decode image: {:?}", e));
                                                warn!("Failed to decode image");
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        crate::backend::log_to_file(&format!("Media Loop: Failed to get response bytes: {:?}", e));
                                    }
                                }
                            }
                            Err(e) => {
                                crate::backend::log_to_file(&format!("Media Loop: reqwest get failed: {:?}", e));
                            }
                        }
                    }
                } else if url_changed && art_url.is_empty() {
                    crate::backend::log_to_file("Media Loop: URL changed to empty");
                    last_art_url = String::new();
                }

                let _ = slint::invoke_from_event_loop({
                    let ui_handle = mpris_ui_handle.clone();
                    move || {
                        if let Some(ui) = ui_handle.upgrade() {
                            ui.set_track_title(SharedString::from(title));
                            ui.set_track_artist(SharedString::from(artist));
                            ui.set_is_playing(is_playing);
                            
                            if url_changed {
                                if let Some(rgba) = loaded_image {
                                    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                                        rgba.as_raw(),
                                        rgba.width(),
                                        rgba.height(),
                                    );
                                    let slint_img = slint::Image::from_rgba8(buffer);
                                    ui.set_album_art(slint_img);
                                    ui.set_has_album_art(true);
                                } else if art_url.is_empty() {
                                    ui.set_album_art(slint::Image::default());
                                    ui.set_has_album_art(false);
                                }
                            }
                        }
                    }
                });
            }
        }
    });
}

fn extract_youtube_id(url_str: &str) -> Option<String> {
    if let Ok(parsed) = url::Url::parse(url_str) {
        let host = parsed.host_str()?;
        if host.contains("youtube.com") || host.contains("youtu.be") {
            if host.contains("youtu.be") {
                return parsed.path_segments()?
                    .next()
                    .map(|s| s.to_string());
            } else {
                if parsed.path().starts_with("/watch") {
                    for (key, val) in parsed.query_pairs() {
                        if key == "v" {
                            return Some(val.into_owned());
                        }
                    }
                } else if parsed.path().starts_with("/embed/") {
                    return parsed.path_segments()?
                        .nth(1)
                        .map(|s| s.to_string());
                } else if parsed.path().starts_with("/v/") {
                    return parsed.path_segments()?
                        .nth(1)
                        .map(|s| s.to_string());
                }
            }
        }
    }
    None
}

async fn search_youtube_for_id(title: &str) -> Option<String> {
    let clean_title = title.replace(" - YouTube", "").trim().to_string();
    let encoded: String = clean_title.chars().map(|c| {
        if c.is_alphanumeric() {
            c.to_string()
        } else if c == ' ' {
            "+".to_string()
        } else {
            format!("%{:02X}", c as u32)
        }
    }).collect();
    
    let url = format!("https://www.youtube.com/results?search_query={}", encoded);
    crate::backend::log_to_file(&format!("Media Search: Querying YouTube search URL: {}", url));
    
    if let Ok(resp) = reqwest::get(&url).await {
        if let Ok(html) = resp.text().await {
            if let Some(idx) = html.find("\"videoId\":\"") {
                let start = idx + 11;
                if html.len() >= start + 11 {
                    let id = &html[start..start + 11];
                    if id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                        crate::backend::log_to_file(&format!("Media Search: Extracted video ID from HTML: {}", id));
                        return Some(id.to_string());
                    }
                }
            }
        }
    }
    None
}
