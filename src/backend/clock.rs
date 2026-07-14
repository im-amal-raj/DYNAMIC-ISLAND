use std::time::Duration;
use std::collections::HashSet;
use chrono::{Local, Datelike, Timelike};
use notify_rust::{Notification, Urgency};
use slint::SharedString;
use crate::{IslandSection, UltimateDynamicIsland};
use super::database::{TaskDatabase, save_db, refresh_calendar_grid};

pub fn start_clock_and_alert_loop(
    ui_weak: slint::Weak<UltimateDynamicIsland>,
    db_container: std::sync::Arc<std::sync::Mutex<TaskDatabase>>,
    refresh_tasks_ui: std::sync::Arc<dyn Fn() + Send + Sync + 'static>,
) {
    tokio::spawn(async move {
        let mut notified_tasks = HashSet::new();
        loop {
            let current_time = Local::now();
            let date_str = format!("{} {} {}", 
                current_time.format("%b"), 
                current_time.day(), 
                current_time.format("%a")
            );
            let time_str = format!("{}", current_time.format("%I:%M %p"));

            let _ = slint::invoke_from_event_loop({
                let ui_handle = ui_weak.clone();
                let time_str_clone = time_str.clone();
                let date_str_clone = date_str.clone();
                move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_current_date_string(SharedString::from(&date_str_clone));
                        ui.set_current_time_string(SharedString::from(&time_str_clone));
                    }
                }
            });

            let current_date_str = format!("{:04}-{:02}-{:02}", current_time.year(), current_time.month(), current_time.day());
            let current_hm_str = format!("{:02}:{:02}", current_time.hour(), current_time.minute());

            let mut tasks_to_notify = Vec::new();
            if let Ok(db) = db_container.lock() {
                for task in &db.items {
                    if !task.is_completed 
                       && task.due_date_str.to_string() == current_date_str 
                       && task.due_time_str.to_string() == current_hm_str 
                       && !notified_tasks.contains(&task.id) 
                    {
                        tasks_to_notify.push((task.id, task.title.to_string(), task.priority.to_string()));
                    }
                }
            }

            for (id, title, priority) in tasks_to_notify {
                notified_tasks.insert(id);
                
                let ui_handle = ui_weak.clone();
                let title_clone = title.clone();
                let priority_clone = priority.clone();
                let refresh_tasks_ui_clone = refresh_tasks_ui.clone();
                let db_container_clone = db_container.clone();

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        if ui.get_chime_on_due() {
                            let _ = std::process::Command::new("paplay")
                                .arg("/usr/share/sounds/freedesktop/stereo/message.oga")
                                .spawn();
                        }
                        ui.set_notif_app_name(SharedString::from("Tasks Planner"));
                        ui.set_notif_summary(SharedString::from("Task Due!"));
                        ui.set_notif_body(SharedString::from(&title_clone));
                        ui.set_has_notif_icon(false);
                        ui.set_active_section(IslandSection::Notification);
                        ui.set_target_width(380.0);
                        ui.set_target_height(110.0);

                        let urgency = match priority_clone.as_str() {
                            "High" => Urgency::Critical,
                            _ => Urgency::Normal,
                        };
                        let notif_handle = Notification::new()
                            .summary("Task Due!")
                            .body(&title_clone)
                            .urgency(urgency)
                            .action("complete", "Complete")
                            .action("snooze", "Snooze 10m")
                            .show();
                        
                        if let Ok(h) = notif_handle {
                            let db_clone = db_container_clone.clone();
                            let ui_weak_notif = ui_handle.clone();
                            let refresh_ui = refresh_tasks_ui_clone.clone();
                            tokio::spawn(async move {
                                h.wait_for_action(move |action| {
                                    match action {
                                        "complete" => {
                                            {
                                                let mut db = db_clone.lock().unwrap();
                                                if let Some(t) = db.items.iter_mut().find(|task| task.id == id) {
                                                    t.is_completed = true;
                                                }
                                                save_db(&db);
                                            }
                                            let db_clone = db_clone.clone();
                                            let ui_weak = ui_weak_notif.clone();
                                            let refresh_ui = refresh_ui.clone();
                                            let _ = slint::invoke_from_event_loop(move || {
                                                if let Some(ui) = ui_weak.upgrade() {
                                                    if let Ok(db) = db_clone.lock() {
                                                        refresh_calendar_grid(&ui, &db);
                                                    }
                                                    refresh_ui();
                                                }
                                            });
                                        }
                                        "snooze" => {
                                            {
                                                let mut db = db_clone.lock().unwrap();
                                                if let Some(t) = db.items.iter_mut().find(|task| task.id == id) {
                                                    let mut time_parts = t.due_time_str.split(':');
                                                    if let (Some(h_str), Some(m_str)) = (time_parts.next(), time_parts.next()) {
                                                        if let (Ok(h), Ok(m)) = (h_str.parse::<u32>(), m_str.parse::<u32>()) {
                                                            let mut new_m = m + 10;
                                                            let mut new_h = h;
                                                            if new_m >= 60 {
                                                                new_m -= 60;
                                                                new_h += 1;
                                                                if new_h >= 24 {
                                                                    new_h = 0;
                                                                }
                                                            }
                                                            t.due_time_str = format!("{:02}:{:02}", new_h, new_m);
                                                        }
                                                    }
                                                }
                                                save_db(&db);
                                            }
                                            let refresh_ui = refresh_ui.clone();
                                            let _ = slint::invoke_from_event_loop(move || {
                                                refresh_ui();
                                            });
                                        }
                                        _ => {}
                                    }
                                });
                            });
                        }
                    }
                });

                let ui_handle_expire = ui_weak.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(ui) = ui_handle_expire.upgrade() {
                            if ui.get_active_section() == IslandSection::Notification {
                                ui.set_active_section(IslandSection::Compact);
                                ui.set_target_width(220.0);
                                ui.set_target_height(42.0);
                            }
                        }
                    });
                });
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}
