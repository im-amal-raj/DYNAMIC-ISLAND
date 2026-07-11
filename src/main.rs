mod backend;

use std::time::Duration;
use std::rc::Rc;
use slint::{ComponentHandle, SharedString, ModelRc, VecModel};
use std::sync::{Arc, Mutex};
use chrono::{Datelike, Timelike};
use rodio::Source;
use log::{info, warn};


// Tray moved to backend::tray

// Notifications moved to backend::notifications

slint::include_modules!();

// Database and calendar logic moved to backend::database

struct IslandControl {
    ui_handle: slint::Weak<UltimateDynamicIsland>,
}

#[zbus::interface(name = "org.rustisland.Control")]
impl IslandControl {
    async fn toggle_launcher(&self) {
        let _ = slint::invoke_from_event_loop({
            let ui_handle = self.ui_handle.clone();
            move || {
                if let Some(ui) = ui_handle.upgrade() {
                    let current = ui.get_active_section();
                    if current == IslandSection::AppLauncher {
                        ui.set_active_section(IslandSection::Compact);
                    } else {
                        ui.set_active_section(IslandSection::AppLauncher);
                    }
                }
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    info!("Starting rust-island...");
    std::env::set_var("SLINT_BACKEND", "winit");

    // Establish DBus connection first to check if we are already running
    let dbus_conn = zbus::Connection::session().await?;
    
    // Try to claim the daemon name
    let is_daemon_owner = match dbus_conn.request_name_with_flags(
        "org.rustisland.Daemon",
        zbus::fdo::RequestNameFlags::AllowReplacement.into(),
    ).await {
        Ok(reply) => {
            matches!(reply, zbus::fdo::RequestNameReply::PrimaryOwner)
        }
        Err(_) => false,
    };
    
    if !is_daemon_owner {
        info!("Another instance of rust-island is already running. Toggling launcher...");
        let proxy = zbus::Proxy::new(
            &dbus_conn,
            "org.rustisland.Daemon",
            "/Control",
            "org.rustisland.Control",
        ).await;
        if let Ok(p) = proxy {
            let _: Result<(), zbus::Error> = p.call("ToggleLauncher", &()).await;
        }
        return Ok(());
    }

    let ui = UltimateDynamicIsland::new()?;
    let ui_handle = ui.as_weak();
    
    // Register the control object
    let control = IslandControl {
        ui_handle: ui_handle.clone(),
    };
    dbus_conn.object_server().at("/Control", control).await?;
    
    let initial_db = backend::database::load_db();
    let db_container = Arc::new(Mutex::new(initial_db));

    backend::database::refresh_calendar_grid(&ui, &db_container.lock().unwrap());
    backend::database::update_ui_task_model(&ui, &db_container.lock().unwrap().items);

    ui.on_play_alert_sound(move || {
        std::thread::spawn(move || {
            if let Ok((_stream, handle)) = rodio::OutputStream::try_default() {
                if let Ok(sink) = rodio::Sink::try_new(&handle) {
                    let alert_source = rodio::source::SineWave::new(880.0)
                        .take_duration(Duration::from_millis(120))
                        .amplify(0.20);
                    sink.append(alert_source);
                    sink.sleep_until_end();
                }
            }
        });
    });

    let completion_db = db_container.clone();
    let ui_weak = ui.as_weak();
    ui.on_toggle_task_completion(move |id| {
        if let Some(ui) = ui_weak.upgrade() {
            let mut db = completion_db.lock().unwrap();
            if let Some(task) = db.items.iter_mut().find(|t| t.id == id) {
                task.is_completed = !task.is_completed;
            }
            backend::database::refresh_calendar_grid(&ui, &db);
            backend::database::update_ui_task_model(&ui, &db.items);
            backend::database::save_db(&db);
        }
    });

    let reschedule_db = db_container.clone();
    let ui_weak2 = ui.as_weak();
    ui.on_reschedule_task(move |id, new_date| {
        if let Some(ui) = ui_weak2.upgrade() {
            let mut db = reschedule_db.lock().unwrap();
            if let Some(task) = db.items.iter_mut().find(|t| t.id == id) {
                if new_date == "+1 day" {
                    use chrono::NaiveDate;
                    if let Ok(mut nd) = NaiveDate::parse_from_str(&task.due_date_str, "%Y-%m-%d") {
                        nd = nd.checked_add_signed(chrono::Duration::days(1)).unwrap_or(nd);
                        task.due_date_str = nd.format("%Y-%m-%d").to_string();
                    } else {
                        task.due_date_str = chrono::Local::now().checked_add_signed(chrono::Duration::days(1)).unwrap().format("%Y-%m-%d").to_string();
                    }
                } else {
                    task.due_date_str = new_date.to_string();
                }
            }
            backend::database::save_db(&db);
            backend::database::update_ui_task_model(&ui, &db.items);
        }
    });

    let filter_db = db_container.clone();
    let ui_weak3 = ui.as_weak();
    ui.on_filter_tasks_by_date(move |date_target| {
        if let Some(ui) = ui_weak3.upgrade() {
            let db = filter_db.lock().unwrap();
            let day_num: u32 = date_target.to_string().parse().unwrap_or(0);
            let target_str = format!("{:04}-{:02}-{:02}", db.view_year, db.view_month, day_num);
            
            let filtered_subset: Vec<backend::database::SavedTask> = db.items.iter()
                .filter(|t| t.due_date_str == target_str)
                .cloned()
                .collect();
            backend::database::update_ui_task_model(&ui, &filtered_subset);
        }
    });

    let add_db = db_container.clone();
    let add_ui_weak = ui.as_weak();
    ui.on_add_task(move |task_text, date_text| {
        if let Some(ui) = add_ui_weak.upgrade() {
            let mut db = add_db.lock().unwrap();
            let new_id = db.items.len() as i32 + 1;
            let current_date = chrono::Local::now();
            let mut final_date_str = format!("{:04}-{:02}-{:02}", current_date.year(), current_date.month(), current_date.day());
            let date_str = date_text.to_string().to_lowercase();
            
            if date_str == "tomorrow" {
                if let Some(tomorrow) = current_date.checked_add_signed(chrono::Duration::days(1)) {
                    final_date_str = format!("{:04}-{:02}-{:02}", tomorrow.year(), tomorrow.month(), tomorrow.day());
                }
            } else if date_str == "next-monday" {
                let mut days_to_add = 1;
                while (current_date + chrono::Duration::days(days_to_add)).weekday() != chrono::Weekday::Mon {
                    days_to_add += 1;
                }
                if let Some(next_monday) = current_date.checked_add_signed(chrono::Duration::days(days_to_add)) {
                    final_date_str = format!("{:04}-{:02}-{:02}", next_monday.year(), next_monday.month(), next_monday.day());
                }
            } else if date_str.len() == 10 && date_str.contains('-') {
                final_date_str = date_str;
            } else if !date_str.is_empty() && date_str != "yyyy-mm-dd" && date_str != "today" {
                if let Ok(day_num) = date_str.parse::<u32>() {
                    final_date_str = format!("{:04}-{:02}-{:02}", db.view_year, db.view_month, day_num);
                }
            }
            
            db.items.push(backend::database::SavedTask {
                id: new_id,
                title: task_text.to_string(),
                due_date_str: final_date_str,
                due_time_str: "23:59".to_string(),
                is_completed: false,
            });
            backend::database::refresh_calendar_grid(&ui, &db);
            backend::database::update_ui_task_model(&ui, &db.items);
            backend::database::save_db(&db);
        }
    });

    let clear_db = db_container.clone();
    let clear_ui_weak = ui.as_weak();
    ui.on_clear_completed_tasks(move || {
        if let Some(ui) = clear_ui_weak.upgrade() {
            let mut db = clear_db.lock().unwrap();
            db.items.retain(|t| !t.is_completed);
            backend::database::refresh_calendar_grid(&ui, &db);
            backend::database::update_ui_task_model(&ui, &db.items);
            backend::database::save_db(&db);
        }
    });

    let nav_db = db_container.clone();
    let nav_ui_weak = ui.as_weak();
    ui.on_change_month_offset(move |offset| {
        if let Some(ui) = nav_ui_weak.upgrade() {
            let mut db = nav_db.lock().unwrap();
            let mut new_month = db.view_month as i32 + offset;
            let mut new_year = db.view_year;
            if new_month < 1 {
                new_month = 12;
                new_year -= 1;
            } else if new_month > 12 {
                new_month = 1;
                new_year += 1;
            }
            db.view_month = new_month as u32;
            db.view_year = new_year;
            backend::database::refresh_calendar_grid(&ui, &db);
        }
    });
    
    backend::apps::load_desktop_apps();
    
    let search_ui_handle = ui_handle.clone();
    ui.on_search_apps(move |query| {
        if let Some(ui) = search_ui_handle.upgrade() {
            let mut results = Vec::new();
            if let Ok(cache) = backend::apps::APPS_CACHE.lock() {
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

    let launch_ui_handle = ui_handle.clone();
    ui.on_launch_app(move |cmd| {
        let _ = std::process::Command::new("sh").arg("-c").arg(format!("{} &", cmd)).spawn();
        if let Some(ui) = launch_ui_handle.upgrade() {
            ui.set_active_section(IslandSection::Compact);
        }
    });
    
    let clear_notifs_ui_handle = ui_handle.clone();
    ui.on_clear_notif_history(move || {
        if let Ok(mut cache) = backend::notifications::NOTIFS_CACHE.lock() {
            cache.clear();
        }
        if let Some(ui) = clear_notifs_ui_handle.upgrade() {
            ui.set_notif_history(ModelRc::default());
        }
    });
    
    let notif_ui_handle = ui_handle.clone();
    tokio::spawn(async move {
        let notif_server = backend::notifications::NotificationServer { 
            ui_handle: notif_ui_handle,
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
                info!("Successfully registered notification server on DBus");
                crate::backend::log_to_file("Notif Init: Successfully registered notification server on DBus");
                // Keep the connection and the name alive
                let _keep_alive = conn;
                std::future::pending::<()>().await;
            }
            Err(e) => {
                warn!("Failed to register DBus notification server (another daemon might be running): {:?}", e);
                crate::backend::log_to_file(&format!("Notif Init: Failed to register DBus notification server: {:?}", e));
            }
        }
    });
    backend::media::setup_media(&ui);
    backend::clipboard::setup_clipboard(&ui);
    backend::hardware::setup_hardware(&ui);

    // Media polling loop moved to backend::media

    let clock_ui_handle = ui_handle.clone();
    let clock_db_container = db_container.clone();
    tokio::spawn(async move {
        let mut notified_tasks = std::collections::HashSet::new();
        loop {
            let current_time = chrono::Local::now();
            let time_str = format!("{} {} {} • {}", 
                current_time.format("%b"), 
                current_time.day(), 
                current_time.format("%a"), 
                current_time.format("%I:%M %p")
            );

            let _ = slint::invoke_from_event_loop({
                let ui_handle = clock_ui_handle.clone();
                let time_str_clone = time_str.clone();
                move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_current_time_string(SharedString::from(&time_str_clone));
                    }
                }
            });

            let current_date_str = format!("{:04}-{:02}-{:02}", current_time.year(), current_time.month(), current_time.day());
            let current_hm_str = format!("{:02}:{:02}", current_time.hour(), current_time.minute());

            let mut tasks_to_notify = Vec::new();
            if let Ok(db) = clock_db_container.lock() {
                for task in &db.items {
                    if !task.is_completed 
                       && task.due_date_str.to_string() == current_date_str 
                       && task.due_time_str.to_string() == current_hm_str 
                       && !notified_tasks.contains(&task.id) 
                    {
                        tasks_to_notify.push((task.id, task.title.to_string()));
                    }
                }
            }

            for (id, title) in tasks_to_notify {
                notified_tasks.insert(id);
                let _ = std::process::Command::new("notify-send")
                    .arg("-a")
                    .arg("Dynamic Island Tasks")
                    .arg("Task Due!")
                    .arg(&title)
                    .spawn();
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    ui.on_set_volume(|vol| {
        let _ = std::process::Command::new("wpctl")
            .arg("set-volume")
            .arg("@DEFAULT_AUDIO_SINK@")
            .arg(format!("{}%", vol as u32))
            .spawn();
    });
    ui.on_set_mic_volume(|vol| {
        let _ = std::process::Command::new("wpctl")
            .arg("set-volume")
            .arg("@DEFAULT_AUDIO_SOURCE@")
            .arg(format!("{}%", vol as u32))
            .spawn();
    });
    ui.on_set_brightness(|val| {
        let _ = std::process::Command::new("brightnessctl")
            .arg("set")
            .arg(format!("{}%", val as u32))
            .spawn();
    });
    
    let cycle_ui_weak = ui.as_weak();
    ui.on_cycle_tab(move |offset| {
        if let Some(ui) = cycle_ui_weak.upgrade() {
            let current = ui.get_active_section();
            use crate::slint_generatedUltimateDynamicIsland::IslandSection;
            let tabs = vec![
                IslandSection::SysControls,
                IslandSection::AppLauncher,
                IslandSection::TodoManager,
                IslandSection::SystemTray,
            ];
            if let Some(idx) = tabs.iter().position(|&x| x == current) {
                let mut new_idx = idx as i32 + offset;
                if new_idx < 0 { new_idx = tabs.len() as i32 - 1; }
                if new_idx >= tabs.len() as i32 { new_idx = 0; }
                ui.set_active_section(tabs[new_idx as usize]);
            }
        }
    });
    
    ui.on_connect_wifi(move |ssid, pass| {
        let target_ssid = ssid.split(" (").next().unwrap_or(ssid.as_str());
        let _ = std::process::Command::new("nmcli")
            .arg("dev")
            .arg("wifi")
            .arg("connect")
            .arg(target_ssid)
            .arg("password")
            .arg(pass.as_str())
            .spawn();
    });

    ui.on_disconnect_wifi(|| {
        let _ = std::process::Command::new("nmcli").arg("dev").arg("disconnect").spawn();
    });
    ui.on_scan_wifi(|| {
        let _ = std::process::Command::new("nmcli").arg("dev").arg("wifi").arg("rescan").spawn();
    });
    ui.on_toggle_wifi(|enabled| {
        let state = if enabled { "on" } else { "off" };
        let _ = std::process::Command::new("nmcli").arg("radio").arg("wifi").arg(state).spawn();
    });
    ui.on_scan_bluetooth(|| {
        let _ = std::process::Command::new("bluetoothctl").arg("scan").arg("on").spawn();
    });
    ui.on_toggle_bluetooth(|enabled| {
        let state = if enabled { "power on" } else { "power off" };
        let _ = std::process::Command::new("bluetoothctl").arg(state).spawn();
    });
    ui.on_connect_bluetooth(|mac| {
        let _ = std::process::Command::new("bluetoothctl").arg("connect").arg(mac.as_str()).spawn();
    });
    ui.on_disconnect_bluetooth(|mac| {
        let _ = std::process::Command::new("bluetoothctl").arg("disconnect").arg(mac.as_str()).spawn();
    });

    ui.on_set_default_audio(|id| {
        let _ = std::process::Command::new("wpctl").arg("set-default").arg(id.as_str()).spawn();
    });

    fn parse_wpctl_devices(output: &str, section: &str) -> Vec<crate::slint_generatedUltimateDynamicIsland::AudioDevice> {
        let mut devices = Vec::new();
        let mut in_section = false;
        for line in output.lines() {
            if line.contains(section) {
                in_section = true;
                continue;
            }
            if in_section {
                if line.contains("├─") || line.contains("└─") {
                    if !line.starts_with(" │  *") && !line.starts_with(" │      ") && !line.starts_with("    *") && !line.starts_with("        ") {
                        break;
                    }
                }
                if line.trim().is_empty() { break; }
                
                let is_default = line.contains("*");
                let clean = line.replace("│", "").replace("*", "").trim().to_string();
                if let Some((id_str, rest)) = clean.split_once('.') {
                    let id = id_str.trim().to_string();
                    let name_part = if let Some((n, _)) = rest.split_once('[') {
                        n.trim().to_string()
                    } else {
                        rest.trim().to_string()
                    };
                    if !id.is_empty() && !name_part.is_empty() && id.chars().all(char::is_numeric) {
                        devices.push(crate::slint_generatedUltimateDynamicIsland::AudioDevice {
                            id: SharedString::from(id),
                            name: SharedString::from(name_part),
                            is_default,
                        });
                    }
                }
            }
        }
        devices
    }

    let mixers_ui_handle = ui_handle.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            
            let mut vol_val = 50.0;
            if let Ok(out) = std::process::Command::new("wpctl").args(&["get-volume", "@DEFAULT_AUDIO_SINK@"]).output() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Some(v_str) = s.split_whitespace().nth(1) {
                    if let Ok(v) = v_str.parse::<f32>() {
                        vol_val = v * 100.0;
                    }
                }
            }
            
            let mut mic_val = 50.0;
            if let Ok(out) = std::process::Command::new("wpctl").args(&["get-volume", "@DEFAULT_AUDIO_SOURCE@"]).output() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Some(v_str) = s.split_whitespace().nth(1) {
                    if let Ok(v) = v_str.parse::<f32>() {
                        mic_val = v * 100.0;
                    }
                }
            }
            
            let mut bright_val = 50.0;
            if let Ok(get_out) = std::process::Command::new("brightnessctl").arg("get").output() {
                if let Ok(max_out) = std::process::Command::new("brightnessctl").arg("max").output() {
                    let get_s = String::from_utf8_lossy(&get_out.stdout);
                    let max_s = String::from_utf8_lossy(&max_out.stdout);
                    if let (Ok(g), Ok(m)) = (get_s.trim().parse::<f32>(), max_s.trim().parse::<f32>()) {
                        if m > 0.0 {
                            bright_val = (g / m) * 100.0;
                        }
                    }
                }
            }
            
            let mut sinks = Vec::new();
            let mut sources = Vec::new();
            if let Ok(status_out) = std::process::Command::new("wpctl").arg("status").output() {
                let status_s = String::from_utf8_lossy(&status_out.stdout);
                sinks = parse_wpctl_devices(&status_s, "Sinks:");
                sources = parse_wpctl_devices(&status_s, "Sources:");
            }
            
            let _ = slint::invoke_from_event_loop({
                let ui_handle = mixers_ui_handle.clone();
                move || {
                    let rc_sinks = ModelRc::from(Rc::new(VecModel::from(sinks)));
                    let rc_sources = ModelRc::from(Rc::new(VecModel::from(sources)));
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_volume_level(vol_val);
                        ui.set_mic_level(mic_val);
                        ui.set_brightness_level(bright_val);
                        ui.set_audio_sinks(rc_sinks);
                        ui.set_audio_sources(rc_sources);
                    }
                }
            });
        }
    });

    let tray_items = Arc::new(Mutex::new(Vec::new()));
    let watcher = backend::tray::CustomStatusNotifierWatcher {
        items: tray_items.clone(),
    };
    
    let tray_ui_handle = ui_handle.clone();
    let tray_conn = dbus_conn.clone();
    tokio::spawn(async move {
        let dbus_conn = tray_conn;

        // Forcefully hijack the Watcher name to bypass broken system daemons (like frozen waybar/ironbar watchers)
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
            info!("Claimed StatusNotifierWatcher, hosting local watcher object...");
            let _ = dbus_conn.object_server().at("/StatusNotifierWatcher", watcher).await;
            
            // Emit StatusNotifierHostRegistered signal so that running apps register their icons
            let server = dbus_conn.object_server();
            if let Ok(iface_ref) = server.interface::<_, backend::tray::CustomStatusNotifierWatcher>("/StatusNotifierWatcher").await {
                let ctx = iface_ref.signal_context();
                let _ = backend::tray::emit_host_registered(ctx).await;
                crate::backend::log_to_file("Tray Init: Emitted StatusNotifierHostRegistered signal to notify running apps");
            } else {
                crate::backend::log_to_file("Tray Init: Failed to get CustomStatusNotifierWatcher interface ref");
            }

        } else {
            info!("StatusNotifierWatcher is already owned by the system, acting as a passive client.");
        }
            
        info!("Starting System Tray polling loop...");
        let mut loop_counter = 0;
        loop {
            loop_counter += 1;
            
            if is_primary_owner && loop_counter % 2 == 0 {
                let server = dbus_conn.object_server();
                if let Ok(iface_ref) = server.interface::<_, backend::tray::CustomStatusNotifierWatcher>("/StatusNotifierWatcher").await {
                    let ctx = iface_ref.signal_context();
                    let _ = backend::tray::emit_host_registered(ctx).await;
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
                            info!("DBus Watcher returned {} RegisteredStatusNotifierItems", sys_items.len());
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
                                    info!("Discovered new tray item from system watcher: {}", formatted);
                                    lock.push(formatted);
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            crate::backend::log_to_file(&format!("Tray Loop: Failed to get RegisteredStatusNotifierItems: {:?}", e));
                            warn!("Failed to get RegisteredStatusNotifierItems from DBus Watcher: {:?}", e);
                        }
                        Err(_) => {
                            crate::backend::log_to_file("Tray Loop: Timeout waiting for RegisteredStatusNotifierItems");
                            warn!("Timeout waiting for DBus Watcher to return RegisteredStatusNotifierItems");
                        }
                    }
                } else {
                    crate::backend::log_to_file("Tray Loop: Failed to build proxy for org.kde.StatusNotifierWatcher");
                    warn!("Failed to build proxy for org.kde.StatusNotifierWatcher");
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
                info!("Polling {} tray items...", items.len());
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
                            
                            // Fallback to IconPixmap if we couldn't get a standard icon path
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
                                warn!("Tray item {} missing both Title and Icon, marking as dead", item);
                                dead_services.push(item.clone());
                            } else {
                                crate::backend::log_to_file(&format!(
                                    "Tray Loop: Item='{}' resolved: Title='{}', IconName='{}', IconPath='{}'",
                                    item, title, icon_name, icon_path
                                ));
                                info!("Tray item {} resolved to: title='{}', icon_name='{}'", item, title, icon_name);
                                app_info.push((item.clone(), title, icon_path));
                            }
                        }
                        Err(e) => {
                            crate::backend::log_to_file(&format!("Tray Loop: Failed to create proxy for {}: {:?}", item, e));
                            warn!("Failed to create DBus proxy for {}: {:?}", item, e);
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
                let ui_handle = tray_ui_handle.clone();
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
                                id: SharedString::from(&service),
                                name: SharedString::from(&name),
                                exec_cmd: SharedString::from(""),
                                has_icon: !path.is_empty(),
                                icon: img,
                            });
                        }
                        ui.set_system_tray_apps(ModelRc::from(std::rc::Rc::new(VecModel::from(slint_items))));
                    }
                }
            });

            tokio::time::sleep(Duration::from_secs(3)).await;
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

    let conn_ui_handle = ui_handle.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            
            let wifi_out = std::process::Command::new("nmcli").arg("-t").arg("-f").arg("SSID,SIGNAL,ACTIVE").arg("dev").arg("wifi").output();
            let mut networks = Vec::new();
            if let Ok(out) = wifi_out {
                let lines = String::from_utf8_lossy(&out.stdout);
                for line in lines.lines() {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 3 && !parts[0].is_empty() {
                        let ssid = parts[0].to_string();
                        let signal = parts[1].parse::<i32>().unwrap_or(0);
                        let connected = parts[2] == "yes";
                        networks.push((ssid, signal, connected));
                    }
                }
            }
            
            let wifi_enabled_out = std::process::Command::new("nmcli").arg("radio").arg("wifi").output();
            let wifi_enabled = if let Ok(out) = wifi_enabled_out {
                String::from_utf8_lossy(&out.stdout).trim() == "enabled"
            } else { false };
            
            let bt_out = std::process::Command::new("bluetoothctl").arg("show").output();
            let bt_enabled = if let Ok(out) = bt_out {
                String::from_utf8_lossy(&out.stdout).contains("Powered: yes")
            } else { false };
            
            let bt_devices_out = std::process::Command::new("bluetoothctl").arg("devices").output();
            let mut bt_devices = Vec::new();
            if let Ok(out) = bt_devices_out {
                let lines = String::from_utf8_lossy(&out.stdout);
                for line in lines.lines() {
                    let parts: Vec<&str> = line.splitn(3, ' ').collect();
                    if parts.len() == 3 && parts[0] == "Device" {
                        let mac = parts[1].to_string();
                        let name = parts[2].to_string();
                        
                        let info_out = std::process::Command::new("bluetoothctl").arg("info").arg(&mac).output();
                        let is_connected = if let Ok(info) = info_out {
                            String::from_utf8_lossy(&info.stdout).contains("Connected: yes")
                        } else { false };
                        
                        bt_devices.push((name, mac, is_connected));
                    }
                }
            }
            
            let eth_out = std::process::Command::new("nmcli").arg("-t").arg("-f").arg("DEVICE,TYPE,STATE").arg("dev").output();
            let mut eth_name = String::new();
            if let Ok(out) = eth_out {
                for line in String::from_utf8_lossy(&out.stdout).lines() {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() == 3 && parts[1] == "ethernet" && parts[2] == "connected" {
                        eth_name = parts[0].to_string();
                        break;
                    }
                }
            }
            
            let _ = slint::invoke_from_event_loop({
                let ui_handle = conn_ui_handle.clone();
                move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        let wifi_models: Vec<WifiNetwork> = networks.iter().map(|(ssid, signal, connected)| WifiNetwork {
                            ssid: SharedString::from(ssid),
                            signal: *signal,
                            connected: *connected,
                        }).collect();
                        ui.set_wifi_networks(ModelRc::from(Rc::new(VecModel::from(wifi_models))));
                        
                        let bt_models: Vec<BluetoothDevice> = bt_devices.iter().map(|(name, mac, connected)| BluetoothDevice {
                            name: SharedString::from(name),
                            mac: SharedString::from(mac),
                            connected: *connected,
                        }).collect();
                        ui.set_bluetooth_devices(ModelRc::from(Rc::new(VecModel::from(bt_models))));
                        
                        ui.set_wifi_enabled(wifi_enabled);
                        ui.set_bluetooth_enabled(bt_enabled);
                        ui.set_ethernet_name(SharedString::from(&eth_name));
                    }
                }
            });
        }
    });

    ui.run()?;
    Ok(())
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
