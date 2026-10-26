use crate::{UltimateDynamicIsland, IslandSection};

pub fn focus_island_window() {
    let script = r#"
        if command -v umbriel >/dev/null 2>&1; then
            win_id=$(umbriel windows 2>/dev/null | grep -iE 'dynamic-island|rust-island' | awk '{print $1}' | tr -d ':' | head -n 1)
            if [ -n "$win_id" ]; then
                umbriel msg "window-focus:$win_id"
            fi
        elif command -v niri >/dev/null 2>&1; then
            niri msg action focus-window --id $(niri msg --json windows 2>/dev/null | jq '.[] | select(.title=="dynamic-island") | .id' | head -n 1)
        fi
    "#;
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .spawn();
}

pub fn setup(ui_handle: slint::Weak<UltimateDynamicIsland>) {
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;

        // 1. Try Umbriel first (Noctalia v5 native compositor)
        let umbriel_child = tokio::process::Command::new("umbriel")
            .args(&["subscribe", "windows"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();

        if let Ok(mut child) = umbriel_child {
            if let Some(stdout) = child.stdout.take() {
                let mut reader = tokio::io::BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    handle_window_event(&line, &ui_handle);
                }
            }
        }

        // 2. Fallback to Niri if Umbriel was not running
        let niri_child = tokio::process::Command::new("niri")
            .args(&["msg", "-j", "event-stream"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();

        if let Ok(mut child) = niri_child {
            if let Some(stdout) = child.stdout.take() {
                let mut reader = tokio::io::BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    handle_window_event(&line, &ui_handle);
                }
            }
        }
    });
}

fn handle_window_event(line: &str, ui_handle: &slint::Weak<UltimateDynamicIsland>) {
    let mut is_our_window = false;
    let mut has_focus_event = false;

    // Check JSON structured events (Umbriel / Niri)
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(line) {
        // Niri WindowFocusChanged
        if let Some(focus_changed) = parsed.get("WindowFocusChanged") {
            has_focus_event = true;
            if !focus_changed.is_null() {
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
            }
        }
        // Umbriel Window Focus / Event
        else if let Some(windows) = parsed.as_array() {
            has_focus_event = true;
            for win in windows {
                let is_focused = win.get("is_focused").and_then(|v| v.as_bool()).unwrap_or(false)
                    || win.get("focused").and_then(|v| v.as_bool()).unwrap_or(false);
                if is_focused {
                    let app_id = win.get("app_id").and_then(|a| a.as_str()).unwrap_or("");
                    let title = win.get("title").and_then(|t| t.as_str()).unwrap_or("");
                    if app_id.contains("rust-island") || app_id.contains("dynamic-island")
                        || title.contains("rust-island") || title.contains("dynamic-island") {
                        is_our_window = true;
                    }
                }
            }
        } else if let Some(is_focused) = parsed.get("is_focused").and_then(|v| v.as_bool()) {
            if is_focused {
                has_focus_event = true;
                let app_id = parsed.get("app_id").and_then(|a| a.as_str()).unwrap_or("");
                let title = parsed.get("title").and_then(|t| t.as_str()).unwrap_or("");
                if app_id.contains("rust-island") || app_id.contains("dynamic-island")
                    || title.contains("rust-island") || title.contains("dynamic-island") {
                    is_our_window = true;
                }
            }
        }
    }

    if has_focus_event && !is_our_window {
        let ui_weak = ui_handle.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let sec = ui.get_active_section();
                if sec != IslandSection::Compact && sec != IslandSection::Media {
                    ui.set_active_section(IslandSection::Compact);
                }
            }
        });
    }
}
