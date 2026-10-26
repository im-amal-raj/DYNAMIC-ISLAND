use slint::{SharedString, ComponentHandle};
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::Notify;
use crate::UltimateDynamicIsland;
use log::{info, warn};

pub fn setup_media(
    ui: &UltimateDynamicIsland,
    active_section: Arc<AtomicU8>,
    notify: Arc<Notify>,
) {
    let last_interaction = Arc::new(Mutex::new(None));
    let last_interaction_clone = last_interaction.clone();
    
    ui.on_media_action(move |action| {
        if let Ok(mut guard) = last_interaction_clone.lock() {
            *guard = Some(Instant::now());
        }
        
        let action_str = action.to_string();
        tokio::spawn(async move {
            if let Ok(finder) = mpris::PlayerFinder::new() {
                if let Ok(players) = finder.find_all() {
                    if let Some(player) = players.into_iter().next() {
                        let _ = match action_str.as_str() {
                            "play-pause" => player.play_pause(),
                            "next" => player.next(),
                            "previous" => player.previous(),
                            _ => Ok(()),
                        };
                    }
                }
            }
        });
    });

    let mpris_ui_handle = ui.as_weak();
    let last_interaction_thread = last_interaction.clone();
    let media_active_section = active_section.clone();
    let media_notify_clone = notify.clone();
    tokio::spawn(async move {
        let mut last_art_url = String::new();
        let mut is_media_playing = false;
        let mut last_youtube_title = String::new();
        let mut last_youtube_id = String::new();
        let mut last_logged_track = String::new();
        
        loop {
            let section_u8 = media_active_section.load(Ordering::SeqCst);
            // 1 is Media
            let is_media_tab = section_u8 == 1;

            let sleep_dur = if is_media_tab {
                Duration::from_millis(250)
            } else if is_media_playing {
                Duration::from_millis(1000)
            } else {
                Duration::from_millis(2000)
            };

            tokio::select! {
                _ = tokio::time::sleep(sleep_dur) => {}
                _ = media_notify_clone.notified() => {}
            }
            
            let mut extracted = None;
            let mut play_info = None;
            {
                if let Ok(finder) = mpris::PlayerFinder::new() {
                    if let Ok(players) = finder.find_all() {
                        let mut chosen_idx = None;
                        for (idx, p) in players.iter().enumerate() {
                            if let Ok(status) = p.get_playback_status() {
                                if status == mpris::PlaybackStatus::Playing {
                                    chosen_idx = Some(idx);
                                    break;
                                }
                            }
                        }
                        let final_player = if let Some(idx) = chosen_idx {
                            players.into_iter().nth(idx)
                        } else {
                            players.into_iter().next()
                        };
                        
                        if let Some(player) = final_player {
                            if let Ok(metadata) = player.get_metadata() {
                                let title = metadata.title().unwrap_or("Unknown").to_string();
                                let artist = metadata.artists().map(|a| a.join(", ")).unwrap_or_else(|| "Unknown".to_string());
                                let art_url = metadata.art_url().unwrap_or("").to_string();
                                let raw_url = metadata.url().unwrap_or("").to_string();
                                let is_playing = player.get_playback_status().unwrap_or(mpris::PlaybackStatus::Stopped) == mpris::PlaybackStatus::Playing;
                                let bus_name = player.bus_name().to_string();
                                let pos = player.get_position().unwrap_or_default();
                                let len = metadata.length().unwrap_or_default();
                                play_info = Some((bus_name, title, artist, art_url, raw_url, is_playing, pos, len));
                            }
                        }
                    }
                }
            }

            if let Some((bus_name, title, artist, mut art_url, raw_url, is_playing, pos, len)) = play_info {
                let track_sig = format!("{}:{}:{}", bus_name, title, is_playing);
                if track_sig != last_logged_track {
                    crate::backend::log_to_file(&format!(
                        "Media Loop: player={}, title='{}', artist='{}', raw_art_url='{}', raw_url='{}', playing={}",
                        bus_name, title, artist, art_url, raw_url, is_playing
                    ));
                    last_logged_track = track_sig;
                }
                
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
                        if title == last_youtube_title && !last_youtube_id.is_empty() {
                            yt_id = Some(last_youtube_id.clone());
                        } else {
                            yt_id = search_youtube_for_id(&title).await;
                            if let Some(id) = &yt_id {
                                last_youtube_title = title.clone();
                                last_youtube_id = id.clone();
                            }
                        }
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
                        // Empty URLs are common for idle browser players; do not flood the debug log
                    }
                }
                
                info!("MPRIS found {}: title='{}', art_url='{}'", bus_name, title, art_url);
                extracted = Some((title, artist, art_url, is_playing, pos, len));
            }

            let is_playing_state = if let Some((_, _, _, is_playing, _, _)) = &extracted {
                *is_playing
            } else {
                false
            };
            is_media_playing = is_playing_state;

            if let Some((title, artist, art_url, is_playing, pos, len)) = extracted {
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

                let skip_is_playing_update = if let Ok(guard) = last_interaction_thread.lock() {
                    if let Some(time) = *guard {
                        time.elapsed() < Duration::from_millis(1200)
                    } else {
                        false
                    }
                } else {
                    false
                };

                let ui_handle = mpris_ui_handle.clone();
                let title_clone = title.clone();
                let artist_clone = artist.clone();
                let pos_secs = pos.as_secs();
                let len_secs = len.as_secs();
                let pos_str = format!("{}:{:02}", pos_secs / 60, pos_secs % 60);
                let len_str = if len_secs > 0 { format!("{}:{:02}", len_secs / 60, len_secs % 60) } else { "--:--".to_string() };
                let prog = if len_secs > 0 { (pos.as_secs_f64() / len.as_secs_f64()) as f32 } else { 0.0 };

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        let mut state = ui.get_media_state();
                        state.track_title = SharedString::from(title_clone);
                        state.track_artist = SharedString::from(artist_clone);
                        if !skip_is_playing_update {
                            state.is_playing = is_playing;
                            ui.set_is_playing(is_playing);
                        }
                        state.track_position_str = SharedString::from(pos_str);
                        state.track_duration_str = SharedString::from(len_str);
                        state.track_progress = prog;
                        
                        if url_changed {
                            if let Some(rgba) = loaded_image {
                                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                                    rgba.as_raw(),
                                    rgba.width(),
                                    rgba.height(),
                                );
                                let slint_img = slint::Image::from_rgba8(buffer);
                                state.album_art = slint_img;
                                state.has_album_art = true;
                            } else if art_url.is_empty() {
                                state.album_art = slint::Image::default();
                                state.has_album_art = false;
                            }
                        }
                        ui.set_media_state(state);
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
