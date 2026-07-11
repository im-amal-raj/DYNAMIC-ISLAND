use std::sync::Mutex;
use std::collections::HashMap;

#[derive(Clone)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub icon_path: String,
}

pub static APPS_CACHE: Mutex<Vec<DesktopApp>> = Mutex::new(Vec::new());

pub fn resolve_icon_path(icon: &str) -> String {
    if icon.is_empty() { return String::new(); }
    if icon.starts_with('/') {
        if std::path::Path::new(icon).exists() { return icon.to_string(); }
        return String::new();
    }

    static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);
    {
        if let Ok(guard) = CACHE.lock() {
            if let Some(ref map) = *guard {
                if let Some(path) = map.get(icon) {
                    return path.clone();
                }
            }
        }
    }

    let possible_paths = vec![
        format!("/usr/share/icons/hicolor/256x256/apps/{}.png", icon),
        format!("/usr/share/icons/hicolor/scalable/apps/{}.svg", icon),
        format!("/usr/share/pixmaps/{}.png", icon),
        format!("/usr/share/icons/breeze/apps/48/{}.svg", icon),
        format!("/usr/share/icons/Adwaita/256x256/apps/{}.png", icon),
        format!("/usr/share/pixmaps/{}.svg", icon),
    ];
    for p in &possible_paths {
        if std::path::Path::new(p).exists() {
            if let Ok(mut guard) = CACHE.lock() {
                let map = guard.get_or_insert_with(HashMap::new);
                map.insert(icon.to_string(), p.clone());
            }
            return p.clone();
        }
    }

    // Fallback: search system directories recursively
    let search_roots = vec![
        "/usr/share/icons".to_string(),
        format!("{}/.local/share/icons", std::env::var("HOME").unwrap_or_default()),
        format!("{}/.icons", std::env::var("HOME").unwrap_or_default()),
        "/usr/share/pixmaps".to_string(),
    ];

    fn find_file_recursive(dir: &std::path::Path, target_name: &str) -> Option<std::path::PathBuf> {
        if !dir.is_dir() { return None; }
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut subdirs = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    subdirs.push(path);
                } else if path.is_file() {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if stem == target_name {
                            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                if ext == "png" || ext == "svg" || ext == "xpm" {
                                    return Some(path);
                                }
                            }
                        }
                    }
                }
            }
            for subdir in subdirs {
                if let Some(found) = find_file_recursive(&subdir, target_name) {
                    return Some(found);
                }
            }
        }
        None
    }

    for root in search_roots {
        if let Some(found_path) = find_file_recursive(std::path::Path::new(&root), icon) {
            let path_str = found_path.to_string_lossy().to_string();
            if let Ok(mut guard) = CACHE.lock() {
                let map = guard.get_or_insert_with(HashMap::new);
                map.insert(icon.to_string(), path_str.clone());
            }
            return path_str;
        }
    }

    // Cache the failure as empty string to avoid repeating expensive search
    if let Ok(mut guard) = CACHE.lock() {
        let map = guard.get_or_insert_with(HashMap::new);
        map.insert(icon.to_string(), String::new());
    }
    String::new()
}

pub fn load_desktop_apps() {
    let mut apps = Vec::new();
    let dirs = vec![
        "/usr/share/applications".to_string(),
        format!("{}/.local/share/applications", std::env::var("HOME").unwrap_or_default()),
    ];
    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("desktop") {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        let mut name = String::new();
                        let mut exec = String::new();
                        let mut icon = String::new();
                        let mut nodisplay = false;
                        
                        for line in content.lines() {
                            if line.starts_with("Name=") && name.is_empty() { name = line[5..].to_string(); }
                            if line.starts_with("Exec=") && exec.is_empty() { exec = line[5..].to_string(); }
                            if line.starts_with("Icon=") && icon.is_empty() { icon = line[5..].to_string(); }
                            if line.starts_with("NoDisplay=true") { nodisplay = true; }
                        }
                        
                        if !nodisplay && !name.is_empty() && !exec.is_empty() {
                            let exec = exec.split('%').next().unwrap_or(&exec).trim().to_string();
                            apps.push(DesktopApp {
                                id: entry.file_name().to_string_lossy().to_string(),
                                name,
                                exec,
                                icon_path: resolve_icon_path(&icon),
                            });
                        }
                    }
                }
            }
        }
    }
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    if let Ok(mut cache) = APPS_CACHE.lock() { *cache = apps; }
}
