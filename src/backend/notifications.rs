use std::time::Duration;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use std::sync::Mutex;
use std::rc::Rc;
use slint::{SharedString, ModelRc, VecModel};
use zbus::interface;

use crate::{UltimateDynamicIsland, NotifItem, IslandSection};

static NOTIF_COUNTER: AtomicUsize = AtomicUsize::new(0);
static LAST_SOUND_TIME: AtomicU64 = AtomicU64::new(0);

pub struct NotifRaw {
    pub id: String,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub icon_path: String,
}

pub static NOTIFS_CACHE: Mutex<Vec<NotifRaw>> = Mutex::new(Vec::new());

pub struct NotificationServer {
    pub ui_handle: slint::Weak<UltimateDynamicIsland>,
    pub rt_handle: tokio::runtime::Handle,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    async fn notify(
        &self,
        app_name: &str,
        _replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        _actions: Vec<&str>,
        _hints: HashMap<&str, zbus::zvariant::Value<'_>>,
        _expire_timeout: i32,
    ) -> u32 {
        let app_name = app_name.to_string();
        let summary = summary.to_string();
        let body = body.to_string();
        
        let current_notif_id = NOTIF_COUNTER.fetch_add(1, Ordering::SeqCst);
        
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
        let last = LAST_SOUND_TIME.load(Ordering::SeqCst);
        if now - last > 2000 {
            if app_name.to_lowercase() != "discord" {
                let _ = std::process::Command::new("paplay")
                    .arg("/usr/share/sounds/freedesktop/stereo/message.oga")
                    .spawn();
            }
            LAST_SOUND_TIME.store(now, Ordering::SeqCst);
        }
        
        let mut has_icon = false;
        let mut icon_path_str = String::new();
        
        if let Some(zbus::zvariant::Value::Str(path)) = _hints.get("image-path") {
            if std::path::Path::new(path.as_str()).exists() {
                icon_path_str = path.as_str().to_string();
                has_icon = true;
            }
        }
        
        if !has_icon && !app_icon.is_empty() {
            let possible_paths = vec![
                app_icon.to_string(),
                format!("/usr/share/icons/hicolor/256x256/apps/{}.png", app_icon),
                format!("/usr/share/icons/hicolor/scalable/apps/{}.svg", app_icon),
                format!("/usr/share/pixmaps/{}.png", app_icon),
                format!("/usr/share/icons/hicolor/256x256/apps/{}.png", app_name.to_lowercase()),
                format!("/usr/share/icons/hicolor/scalable/apps/{}.svg", app_name.to_lowercase()),
            ];
            for path in possible_paths {
                if std::path::Path::new(&path).exists() {
                    icon_path_str = path;
                    has_icon = true;
                    break;
                }
            }
        }
        
        let raw = NotifRaw {
            id: format!("{}", current_notif_id),
            app_name: app_name.clone(),
            summary: summary.clone(),
            body: body.clone(),
            icon_path: icon_path_str.clone(),
        };
        if let Ok(mut cache) = NOTIFS_CACHE.lock() {
            cache.insert(0, raw);
            if cache.len() > 50 { cache.truncate(50); }
        }
        
        let ui_handle = self.ui_handle.clone();
        
        let peek_summary = summary.clone();
        let peek_icon_path = icon_path_str.clone();
        
        let _ = slint::invoke_from_event_loop({
            let ui_handle = ui_handle.clone();
            move || {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_notif_app_name(SharedString::from(app_name));
                    ui.set_notif_summary(SharedString::from(summary));
                    ui.set_notif_body(SharedString::from(body));
                    ui.set_has_notif_icon(has_icon);
                    
                    if has_icon && !icon_path_str.is_empty() {
                        if let Ok(img) = slint::Image::load_from_path(std::path::Path::new(&icon_path_str)) {
                            ui.set_notif_icon(img);
                        }
                    }
                    
                    // Update Notification Peek in Compact Mode
                    ui.set_has_new_notification(true);
                    ui.set_peek_notif_summary(SharedString::from(peek_summary));
                    if !peek_icon_path.is_empty() {
                        if let Ok(img) = slint::Image::load_from_path(std::path::Path::new(&peek_icon_path)) {
                            ui.set_peek_notif_icon(img);
                        }
                    } else {
                        ui.set_peek_notif_icon(slint::Image::default());
                    }
                    
                    let mut slint_notifs = Vec::new();
                    if let Ok(cache) = NOTIFS_CACHE.lock() {
                        for n in cache.iter() {
                            let img = if !n.icon_path.is_empty() {
                                slint::Image::load_from_path(std::path::Path::new(&n.icon_path)).unwrap_or_default()
                            } else { slint::Image::default() };
                            slint_notifs.push(NotifItem {
                                id: SharedString::from(&n.id),
                                app_name: SharedString::from(&n.app_name),
                                summary: SharedString::from(&n.summary),
                                body: SharedString::from(&n.body),
                                has_icon: !n.icon_path.is_empty(),
                                icon: img,
                            });
                        }
                    }
                    ui.set_notif_history(ModelRc::from(Rc::new(VecModel::from(slint_notifs))));
                    
                    ui.set_active_section(IslandSection::Notification);
                }
            }
        });
        
        let timeout_ms = if _expire_timeout <= 0 { 5000 } else { _expire_timeout as u64 };
        
        self.rt_handle.spawn(async move {
            tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
            
            let _ = slint::invoke_from_event_loop({
                move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        // Clear peek when notification expires
                        ui.set_has_new_notification(false);
                        
                        if ui.get_active_section() == IslandSection::Notification {
                            ui.set_active_section(IslandSection::Compact);
                        }
                    }
                }
            });
        });
        
        1
    }
    
    async fn get_capabilities(&self) -> Vec<&str> {
        vec!["body"]
    }
    
    async fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("rust-island", "Antigravity", "1.0", "1.2")
    }
    
    async fn close_notification(&self, _id: u32) {
        let ui_handle = self.ui_handle.clone();
        let _ = slint::invoke_from_event_loop({
            move || {
                if let Some(ui) = ui_handle.upgrade() {
                    ui.set_has_new_notification(false);
                    if ui.get_active_section() == IslandSection::Notification {
                        ui.set_active_section(IslandSection::Compact);
                    }
                }
            }
        });
    }
}

pub fn setup_notifications(ui_handle: slint::Weak<UltimateDynamicIsland>) {
    tokio::spawn(async move {
        let notif_server = NotificationServer { 
            ui_handle,
            rt_handle: tokio::runtime::Handle::current(),
        };
        let _conn = zbus::ConnectionBuilder::session()
            .expect("Failed to get DBus session")
            .name("org.freedesktop.Notifications")
            .expect("Failed to claim name")
            .serve_at("/org/freedesktop/Notifications", notif_server)
            .expect("Failed to serve");
            
        match _conn.build().await {
            Ok(conn) => {
                log::info!("Successfully registered notification server on DBus");
                crate::backend::log_to_file("Notif Init: Successfully registered notification server on DBus");
                // Keep the connection and the name alive
                let _keep_alive = conn;
                std::future::pending::<()>().await;
            }
            Err(e) => {
                log::warn!("Failed to register DBus notification server (another daemon might be running): {:?}", e);
                crate::backend::log_to_file(&format!("Notif Init: Failed to register DBus notification server: {:?}", e));
            }
        }
    });
}
