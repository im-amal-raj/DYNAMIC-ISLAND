mod backend;

use slint::ComponentHandle;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU8, Ordering};
use tokio::sync::Notify;

slint::include_modules!();

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
        
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg("niri msg action focus-window --id $(niri msg --json windows | jq '.[] | select(.title==\"dynamic-island\") | .id' | head -n 1)")
            .spawn();
    }
}

fn section_to_u8(sec: IslandSection) -> u8 {
    match sec {
        IslandSection::Compact => 0,
        IslandSection::Media => 1,
        IslandSection::Clipboard => 2,
        IslandSection::Notification => 3,
        IslandSection::SysControls => 4,
        IslandSection::AppLauncher => 5,
        IslandSection::SystemTray => 6,
        IslandSection::NotificationHistory => 7,
        IslandSection::TodoManager => 8,
        IslandSection::CalendarView => 9,
        IslandSection::SysMonitor => 10,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    log::info!("Starting rust-island...");
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
        log::info!("Another instance of rust-island is already running. Toggling launcher...");
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
    let active_section_shared = Arc::new(AtomicU8::new(section_to_u8(ui.get_active_section())));
    
    let mixers_notify = Arc::new(Notify::new());
    let conn_notify = Arc::new(Notify::new());
    let tray_notify = Arc::new(Notify::new());
    let hardware_notify = Arc::new(Notify::new());
    let media_notify = Arc::new(Notify::new());
    
    {
        let active_section_clone = active_section_shared.clone();
        let mixers_n = mixers_notify.clone();
        let conn_n = conn_notify.clone();
        let tray_n = tray_notify.clone();
        let hardware_n = hardware_notify.clone();
        let media_n = media_notify.clone();
        ui.on_active_section_changed(move |sec| {
            active_section_clone.store(section_to_u8(sec), Ordering::SeqCst);
            mixers_n.notify_one();
            conn_n.notify_one();
            tray_n.notify_one();
            hardware_n.notify_one();
            media_n.notify_one();
        });
    }
    
    let ui_handle = ui.as_weak();
    
    // Register the control object
    let control = IslandControl {
        ui_handle: ui_handle.clone(),
    };
    dbus_conn.object_server().at("/Control", control).await?;
    
    let initial_db = backend::database::load_db();
    let db_container = Arc::new(Mutex::new(initial_db));

    let selected_date_filter = Arc::new(std::sync::Mutex::new(String::new()));
    let todo_tab_filter = Arc::new(std::sync::Mutex::new(String::from("Pending")));
    
    let refresh_tasks_ui = backend::database::create_refresh_tasks_ui(
        ui_handle.clone(),
        db_container.clone(),
        selected_date_filter.clone(),
        todo_tab_filter.clone(),
    );
    let refresh_tasks_ui = Arc::new(refresh_tasks_ui);

    backend::database::refresh_calendar_grid(&ui, &db_container.lock().unwrap());
    
    // Initialize tasks list UI with default filter
    refresh_tasks_ui();

    // Spawn modular backend features & loop runners
    backend::notifications::setup_notifications(ui_handle.clone());
    backend::media::setup_media(&ui, active_section_shared.clone(), media_notify.clone());
    backend::clipboard::setup_clipboard(&ui);
    
    // Setup system metrics & connectivity polling loops
    backend::system_stats::setup(&ui, active_section_shared.clone(), hardware_notify.clone());
    backend::audio::setup(&ui, active_section_shared.clone(), mixers_notify.clone());
    backend::brightness::setup(&ui, active_section_shared.clone(), conn_notify.clone());
    backend::wifi::setup(&ui, active_section_shared.clone(), conn_notify.clone());
    backend::bluetooth::setup(&ui, active_section_shared.clone(), conn_notify.clone());

    // Setup system tray watcher & polling loop
    backend::tray::setup_tray(&ui, dbus_conn.clone(), active_section_shared.clone(), tray_notify.clone());

    // Setup clock background loop & calendar alerts
    backend::clock::start_clock_and_alert_loop(ui_handle.clone(), db_container.clone(), refresh_tasks_ui.clone());

    // Register V2 modular handlers (callbacks from UI components)
    backend::handlers::todo::setup(
        &ui,
        db_container.clone(),
        todo_tab_filter.clone(),
        selected_date_filter.clone(),
        refresh_tasks_ui.clone(),
    );
    backend::handlers::system_tray::setup(&ui, dbus_conn.clone());
    backend::handlers::sys_controls::setup(&ui);

    // Setup Niri focus listener for auto-dismiss
    backend::niri::setup(ui_handle.clone());

    ui.run()?;
    Ok(())
}
