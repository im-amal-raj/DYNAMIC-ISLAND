use std::rc::Rc;
use slint::{ComponentHandle, SharedString, ModelRc, VecModel};
use crate::{UltimateDynamicIsland, IslandSection, AppItem};

pub fn setup(
    ui: &UltimateDynamicIsland,
    dbus_conn: zbus::Connection,
) {
    crate::backend::apps::load_desktop_apps();

    // 1. Search desktop apps (App Launcher)
    let search_ui_handle = ui.as_weak();
    ui.on_search_apps(move |query| {
        if let Some(ui) = search_ui_handle.upgrade() {
            let mut results = Vec::new();
            if let Ok(cache) = crate::backend::apps::APPS_CACHE.lock() {
                let q = query.to_lowercase();
                for app in cache.iter() {
                    if query.is_empty() || app.name.to_lowercase().contains(&q) || app.exec.to_lowercase().contains(&q) {
                        let img = if !app.icon_path.is_empty() {
                            slint::Image::load_from_path(std::path::Path::new(&app.icon_path)).unwrap_or_default()
                        } else { slint::Image::default() };
                        
                        results.push(AppItem {
                            id: SharedString::from(&app.id),
                            name: SharedString::from(&app.name),
                            exec_cmd: SharedString::from(&app.exec),
                            has_icon: !app.icon_path.is_empty(),
                            icon: img,
                        });
                        if results.len() >= 10 { break; }
                    }
                }
            }
            ui.set_app_results(ModelRc::from(Rc::new(VecModel::from(results))));
        }
    });
    let _ = ui.invoke_search_apps(SharedString::from(""));

    // 2. Launch selected app
    let launch_ui_handle = ui.as_weak();
    ui.on_launch_app(move |cmd| {
        let _ = std::process::Command::new("sh").arg("-c").arg(format!("{} &", cmd)).spawn();
        if let Some(ui) = launch_ui_handle.upgrade() {
            ui.set_active_section(IslandSection::Compact);
        }
    });
    
    // 3. Power action trigger
    let power_ui_handle = ui.as_weak();
    ui.on_power_action(move |action| {
        let cmd = match action.as_str() {
            "lock" => "loginctl lock-session || hyprlock || swaylock || swaylock-fancy || slock",
            "suspend" => "systemctl suspend",
            "reboot" => "systemctl reboot",
            "poweroff" => "systemctl poweroff",
            _ => "",
        };
        if !cmd.is_empty() {
            let _ = std::process::Command::new("sh").arg("-c").arg(cmd).spawn();
        }
        if let Some(ui) = power_ui_handle.upgrade() {
            ui.set_active_section(IslandSection::Compact);
        }
    });
    
    // 4. Focus main OS window (Wayland/Niri target)
    ui.on_request_window_focus(move || {
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg("niri msg action focus-window --id $(niri msg --json windows | jq '.[] | select(.title==\"dynamic-island\") | .id' | head -n 1)")
            .spawn();
    });
    
    // 5. Clear notification history
    let clear_notifs_ui_handle = ui.as_weak();
    ui.on_clear_notif_history(move || {
        if let Ok(mut cache) = crate::backend::notifications::NOTIFS_CACHE.lock() {
            cache.clear();
        }
        if let Some(ui) = clear_notifs_ui_handle.upgrade() {
            ui.set_notif_history(ModelRc::default());
        }
    });

    // 6. D-Bus status notifier tray icon activation
    let activate_conn = dbus_conn;
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
