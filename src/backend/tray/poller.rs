use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use std::time::Duration;
use slint::ComponentHandle;
use crate::{UltimateDynamicIsland, AppItem};

use super::watcher::{CustomStatusNotifierWatcher, emit_host_registered};

pub fn setup_tray(
    ui: &UltimateDynamicIsland,
    dbus_conn: zbus::Connection,
    active_section: Arc<std::sync::atomic::AtomicU8>,
    notify: Arc<tokio::sync::Notify>,
) {
    let tray_items = Arc::new(Mutex::new(Vec::<String>::new()));
    let watcher = CustomStatusNotifierWatcher {
        items: tray_items.clone(),
    };
    
    let ui_handle = ui.as_weak();
    let tray_conn_clone = dbus_conn.clone();
    let active_section_clone = active_section.clone();
    let notify_clone = notify.clone();
    let tray_items_clone = tray_items.clone();
    
    tokio::spawn(async move {
        let dbus_conn = tray_conn_clone;
        let tray_items = tray_items_clone;

        let is_primary_owner = match dbus_conn.request_name_with_flags(
            "org.kde.StatusNotifierWatcher",
            zbus::fdo::RequestNameFlags::ReplaceExisting.into(),
        ).await {
            Ok(reply) => {
                let status = format!("{:?}", reply).contains("PrimaryOwner") || format!("{:?}", reply).contains("AlreadyOwner");
                crate::backend::log_to_file(&format!("Tray Init: request_name reply={:?}, is_primary_owner={}", reply, status));
                status
            }
            Err(e) => {
                crate::backend::log_to_file(&format!("Tray Init: request_name failed: {:?}", e));
                false
            }
        };
        
        if is_primary_owner {
            log::info!("Claimed StatusNotifierWatcher, hosting local watcher object...");
            let _ = dbus_conn.object_server().at("/StatusNotifierWatcher", watcher).await;
            
            let server = dbus_conn.object_server();
            if let Ok(iface_ref) = server.interface::<_, CustomStatusNotifierWatcher>("/StatusNotifierWatcher").await {
                let ctx = iface_ref.signal_context();
                let _ = emit_host_registered(ctx).await;
                crate::backend::log_to_file("Tray Init: Emitted StatusNotifierHostRegistered signal to notify running apps");
            } else {
                crate::backend::log_to_file("Tray Init: Failed to get CustomStatusNotifierWatcher interface ref");
            }
        } else {
            log::info!("StatusNotifierWatcher is already owned by the system, acting as a passive client.");
        }
            
        log::info!("Starting System Tray polling loop...");
        let mut loop_counter = 0;
        loop {
            let section_u8 = active_section_clone.load(Ordering::SeqCst);
            // 6 is SystemTray
            let is_tray = section_u8 == 6;
            
            if is_tray {
                loop_counter += 1;
                
                if is_primary_owner && loop_counter % 2 == 0 {
                    let server = dbus_conn.object_server();
                    if let Ok(iface_ref) = server.interface::<_, CustomStatusNotifierWatcher>("/StatusNotifierWatcher").await {
                        let ctx = iface_ref.signal_context();
                        let _ = emit_host_registered(ctx).await;
                        crate::backend::log_to_file("Tray Loop: Periodically emitted StatusNotifierHostRegistered signal");
                    }
                }
                
                if !is_primary_owner {
                    let proxy_res: Result<zbus::Proxy, _> = zbus::ProxyBuilder::new(&dbus_conn)
                        .destination("org.kde.StatusNotifierWatcher").unwrap()
                        .path("/StatusNotifierWatcher").unwrap()
                        .interface("org.kde.StatusNotifierWatcher").unwrap()
                        .cache_properties(zbus::CacheProperties::No)
                        .build()
                        .await;
                        
                    if let Ok(proxy) = proxy_res {
                        match tokio::time::timeout(std::time::Duration::from_secs(2), proxy.get_property::<Vec<String>>("RegisteredStatusNotifierItems")).await {
                            Ok(Ok(sys_items)) => {
                                crate::backend::log_to_file(&format!("Tray Loop: DBus Watcher returned raw sys_items={:?}", sys_items));
                                log::info!("DBus Watcher returned {} RegisteredStatusNotifierItems", sys_items.len());
                                let mut lock = tray_items.lock().unwrap();
                                for item in sys_items {
                                    let formatted = if let Some(idx) = item.find('/') {
                                        let (bus, path) = item.split_at(idx);
                                        format!("{}|{}", bus, path)
                                    } else {
                                        format!("{}|/StatusNotifierItem", item)
                                    };
                                    if !lock.contains(&formatted) {
                                        crate::backend::log_to_file(&format!("Tray Loop: Discovered new item from system watcher: {} (original: {})", formatted, item));
                                        log::info!("Discovered new tray item from system watcher: {}", formatted);
                                        lock.push(formatted);
                                    }
                                }
                            }
                            Ok(Err(e)) => {
                                crate::backend::log_to_file(&format!("Tray Loop: Failed to get RegisteredStatusNotifierItems: {:?}", e));
                                log::warn!("Failed to get RegisteredStatusNotifierItems from DBus Watcher: {:?}", e);
                            }
                            Err(_) => {
                                crate::backend::log_to_file("Tray Loop: Timeout waiting for RegisteredStatusNotifierItems");
                                log::warn!("Timeout waiting for DBus Watcher to return RegisteredStatusNotifierItems");
                            }
                        }
                    } else {
                        crate::backend::log_to_file("Tray Loop: Failed to build proxy for org.kde.StatusNotifierWatcher");
                        log::warn!("Failed to build proxy for org.kde.StatusNotifierWatcher");
                    }
                }
                
                let items: Vec<String> = tray_items.lock().unwrap().clone();
                crate::backend::log_to_file(&format!("Tray Loop: Current local tray_items list = {:?}", items));
                
                if let Ok(dbus_proxy) = zbus::fdo::DBusProxy::new(&dbus_conn).await {
                    if let Ok(names) = dbus_proxy.list_names().await {
                        let well_known: Vec<String> = names.into_iter()
                            .map(|n| n.as_str().to_string())
                            .filter(|n| !n.starts_with(':'))
                            .collect();
                        crate::backend::log_to_file(&format!("Tray Loop: Active D-Bus well-known names: {:?}", well_known));
                    }
                }
                
                if !items.is_empty() {
                    log::info!("Polling {} tray items...", items.len());
                }
                
                let mut app_info = Vec::new();
                let mut dead_services = Vec::new();
                
                for item in &items {
                    let parts: Vec<&str> = item.split('|').collect();
                    crate::backend::log_to_file(&format!("Tray Loop: Checking item='{}', split parts={:?}", item, parts));
                    if parts.len() == 2 {
                        let bus_name = parts[0];
                        let path = parts[1];
                        
                        let proxy = zbus::Proxy::new(
                            &dbus_conn,
                            bus_name,
                            path,
                            "org.kde.StatusNotifierItem",
                        ).await;
                        
                        match proxy {
                            Ok(p) => {
                                let title_res = p.get_property::<String>("Title").await;
                                let icon_res = p.get_property::<String>("IconName").await;
                                crate::backend::log_to_file(&format!(
                                    "Tray Loop: Item='{}', Title={:?}, IconName={:?}",
                                    item, title_res, icon_res
                                ));
                                
                                let title = title_res.unwrap_or_else(|_| "App".to_string());
                                let icon_name = icon_res.unwrap_or_default();
                                let mut icon_path = if !icon_name.is_empty() { crate::backend::apps::resolve_icon_path(&icon_name) } else { String::new() };
                                
                                if icon_path.is_empty() {
                                    match p.get_property::<Vec<(i32, i32, Vec<u8>)>>("IconPixmap").await {
                                        Ok(pixmaps) => {
                                            crate::backend::log_to_file(&format!("Tray Loop: Item='{}' fallback: Got IconPixmap with {} items", item, pixmaps.len()));
                                            if let Some((w, h, data)) = pixmaps.into_iter().max_by_key(|(w, h, _)| w * h) {
                                                if let Some(path) = save_pixmap_to_file(w, h, &data) {
                                                    icon_path = path;
                                                    crate::backend::log_to_file(&format!("Tray Loop: Item='{}' fallback: Successfully saved pixmap to {}", item, icon_path));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            crate::backend::log_to_file(&format!("Tray Loop: Item='{}' fallback: Failed to get IconPixmap: {:?}", item, e));
                                        }
                                    }
                                }
                                
                                if title.is_empty() && icon_path.is_empty() {
                                    crate::backend::log_to_file(&format!("Tray Loop: Item='{}' missing Title and IconPixmap/IconName, marking as dead", item));
                                    log::warn!("Tray item {} missing both Title and Icon, marking as dead", item);
                                    dead_services.push(item.clone());
                                } else {
                                    crate::backend::log_to_file(&format!(
                                        "Tray Loop: Item='{}' resolved: Title='{}', IconName='{}', IconPath='{}'",
                                        item, title, icon_name, icon_path
                                    ));
                                    log::info!("Tray item {} resolved to: title='{}', icon_name='{}'", item, title, icon_name);
                                    app_info.push((item.clone(), title, icon_path));
                                }
                            }
                            Err(e) => {
                                crate::backend::log_to_file(&format!("Tray Loop: Failed to create proxy for {}: {:?}", item, e));
                                log::warn!("Failed to create DBus proxy for {}: {:?}", item, e);
                                dead_services.push(item.clone());
                            }
                        }
                    } else {
                        crate::backend::log_to_file(&format!("Tray Loop: Item '{}' has invalid parts len != 2, marking as dead", item));
                        dead_services.push(item.clone());
                    }
                }
                
                if !dead_services.is_empty() {
                    crate::backend::log_to_file(&format!("Tray Loop: Removing dead services: {:?}", dead_services));
                    let mut items_lock = tray_items.lock().unwrap();
                    items_lock.retain(|x| !dead_services.contains(x));
                }
                
                let _ = slint::invoke_from_event_loop({
                    let ui_handle = ui_handle.clone();
                    let app_info_clone = app_info.clone();
                    move || {
                        if let Some(ui) = ui_handle.upgrade() {
                            crate::backend::log_to_file(&format!("Tray UI: Setting system_tray_apps on UI. count={}", app_info_clone.len()));
                            let mut slint_items = Vec::new();
                            for (service, name, path) in app_info_clone {
                                let img = if !path.is_empty() {
                                    slint::Image::load_from_path(std::path::Path::new(&path)).unwrap_or_default()
                                } else { slint::Image::default() };
                                
                                slint_items.push(AppItem {
                                    id: slint::SharedString::from(&service),
                                    name: slint::SharedString::from(&name),
                                    exec_cmd: slint::SharedString::from(""),
                                    has_icon: !path.is_empty(),
                                    icon: img,
                                });
                            }
                            ui.set_system_tray_apps(slint::ModelRc::from(std::rc::Rc::new(slint::VecModel::from(slint_items))));
                        }
                    }
                });
                
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                    _ = notify_clone.notified() => {}
                }
            } else {
                notify_clone.notified().await;
            }
        }
    });

    let activate_conn = dbus_conn.clone();
    ui.on_activate_tray_item(move |service_id| {
        let service = service_id.to_string();
        let conn = activate_conn.clone();
        tokio::spawn(async move {
            let parts: Vec<&str> = service.split('|').collect();
            if parts.len() == 2 {
                let sender = parts[0];
                let path = parts[1];
                let proxy = zbus::Proxy::new(
                    &conn,
                    sender,
                    path,
                    "org.kde.StatusNotifierItem",
                ).await;
                
                if let Ok(proxy) = proxy {
                    let _ = proxy.call::<_, _, ()>("Activate", &(0, 0)).await;
                }
            }
        });
    });
}

fn save_pixmap_to_file(width: i32, height: i32, data: &[u8]) -> Option<String> {
    if width <= 0 || height <= 0 || data.len() != (width * height * 4) as usize {
        return None;
    }
    let mut rgba = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        let a = chunk[0];
        let r = chunk[1];
        let g = chunk[2];
        let b = chunk[3];
        rgba.push(r);
        rgba.push(g);
        rgba.push(b);
        rgba.push(a);
    }
    
    let path = format!("/tmp/tray_pixmap_{}x{}.png", width, height);
    if let Some(img) = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width as u32, height as u32, rgba) {
        if img.save(&path).is_ok() {
            return Some(path);
        }
    }
    None
}
