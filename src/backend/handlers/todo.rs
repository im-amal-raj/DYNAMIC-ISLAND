use std::sync::{Arc, Mutex};
use slint::ComponentHandle;
use crate::UltimateDynamicIsland;
use crate::backend::database::{TaskDatabase, SavedTask, refresh_calendar_grid, save_db};

pub fn setup(
    ui: &UltimateDynamicIsland,
    db_container: Arc<Mutex<TaskDatabase>>,
    todo_tab_filter: Arc<Mutex<String>>,
    selected_date_filter: Arc<Mutex<String>>,
    refresh_tasks_ui: Arc<dyn Fn() + Send + Sync + 'static>,
) {
    // 1. Play alert sound chime
    ui.on_play_alert_sound(move || {
        std::thread::spawn(move || {
            if let Ok((_stream, handle)) = rodio::OutputStream::try_default() {
                if let Ok(sink) = rodio::Sink::try_new(&handle) {
                    use rodio::Source;
                    let alert_source = rodio::source::SineWave::new(880.0)
                        .take_duration(std::time::Duration::from_millis(120))
                        .amplify(0.20);
                    sink.append(alert_source);
                    sink.sleep_until_end();
                }
            }
        });
    });

    // 2. Toggle Task Completion
    let completion_db = db_container.clone();
    let ui_weak = ui.as_weak();
    let refresh_tasks_ui_clone = refresh_tasks_ui.clone();
    ui.on_toggle_task_completion(move |id| {
        if let Some(ui) = ui_weak.upgrade() {
            // Scope block to immediately drop Mutex lock before calling refresh
            {
                let mut db = completion_db.lock().unwrap();
                if let Some(task) = db.items.iter_mut().find(|t| t.id == id) {
                    task.is_completed = !task.is_completed;
                }
                refresh_calendar_grid(&ui, &db);
                save_db(&db);
            }
            refresh_tasks_ui_clone();
        }
    });

    // 3. Reschedule Task
    let reschedule_db = db_container.clone();
    let ui_weak2 = ui.as_weak();
    let refresh_tasks_ui_clone2 = refresh_tasks_ui.clone();
    ui.on_reschedule_task(move |id, new_date| {
        if let Some(ui) = ui_weak2.upgrade() {
            {
                let mut db = reschedule_db.lock().unwrap();
                if let Some(task) = db.items.iter_mut().find(|t| t.id == id) {
                    if new_date == "+1 day" {
                        use chrono::NaiveDate;
                        if let Ok(mut nd) = NaiveDate::parse_from_str(&task.due_date_str, "%Y-%m-%d") {
                            nd = nd.checked_add_signed(chrono::Duration::days(1)).unwrap_or(nd);
                            task.due_date_str = nd.format("%Y-%m-%d").to_string();
                        } else {
                            let current = chrono::Local::now();
                            if let Some(next_day) = current.checked_add_signed(chrono::Duration::days(1)) {
                                use chrono::Datelike;
                                task.due_date_str = format!("{:04}-{:02}-{:02}", next_day.year(), next_day.month(), next_day.day());
                            }
                        }
                    } else {
                        task.due_date_str = new_date.to_string();
                    }
                }
                refresh_calendar_grid(&ui, &db);
                save_db(&db);
            }
            refresh_tasks_ui_clone2();
        }
    });

    // 4. Filter Tasks By Date
    let filter_date_clone = selected_date_filter.clone();
    let refresh_tasks_ui_clone3 = refresh_tasks_ui.clone();
    ui.on_filter_tasks_by_date(move |date_target| {
        let date_str = date_target.to_string();
        *filter_date_clone.lock().unwrap() = date_str;
        refresh_tasks_ui_clone3();
    });

    // 5. Add Task
    let add_db = db_container.clone();
    let add_ui_weak = ui.as_weak();
    let refresh_tasks_ui_clone4 = refresh_tasks_ui.clone();
    ui.on_add_task(move |task_text, date_text, priority_text| {
        if let Some(ui) = add_ui_weak.upgrade() {
            let mut final_priority = priority_text.to_string();
            let mut parsed_date_str = date_text.to_string().to_lowercase();
            let mut parsed_task_text = task_text.to_string();
            
            // Smart tag extraction from task title
            if parsed_task_text.contains('!') {
                if parsed_task_text.contains("!high") {
                    final_priority = "High".to_string();
                    parsed_task_text = parsed_task_text.replace("!high", "").trim().to_string();
                } else if parsed_task_text.contains("!low") {
                    final_priority = "Low".to_string();
                    parsed_task_text = parsed_task_text.replace("!low", "").trim().to_string();
                } else if parsed_task_text.contains("!med") {
                    final_priority = "Medium".to_string();
                    parsed_task_text = parsed_task_text.replace("!med", "").trim().to_string();
                }
            }
            
            if parsed_task_text.contains('@') {
                if parsed_task_text.contains("@today") {
                    let current = chrono::Local::now();
                    use chrono::Datelike;
                    parsed_date_str = format!("{:04}-{:02}-{:02}", current.year(), current.month(), current.day());
                    parsed_task_text = parsed_task_text.replace("@today", "").trim().to_string();
                } else if parsed_task_text.contains("@tomorrow") {
                    let current = chrono::Local::now();
                    if let Some(tomorrow) = current.checked_add_signed(chrono::Duration::days(1)) {
                        use chrono::Datelike;
                        parsed_date_str = format!("{:04}-{:02}-{:02}", tomorrow.year(), tomorrow.month(), tomorrow.day());
                    }
                    parsed_task_text = parsed_task_text.replace("@tomorrow", "").trim().to_string();
                }
            }

            {
                let mut db = add_db.lock().unwrap();
                let new_id = db.items.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                
                let mut final_date_str = String::new();
                if parsed_date_str == "today" {
                    let current_date = chrono::Local::now();
                    use chrono::Datelike;
                    final_date_str = format!("{:04}-{:02}-{:02}", current_date.year(), current_date.month(), current_date.day());
                } else if parsed_date_str == "tomorrow" {
                    let current_date = chrono::Local::now();
                    use chrono::Datelike;
                    if let Some(tomorrow) = current_date.checked_add_signed(chrono::Duration::days(1)) {
                        final_date_str = format!("{:04}-{:02}-{:02}", tomorrow.year(), tomorrow.month(), tomorrow.day());
                    }
                } else if parsed_date_str == "next monday" || parsed_date_str == "monday" {
                    let current_date = chrono::Local::now();
                    use chrono::Datelike;
                    let current_weekday = current_date.weekday().num_days_from_monday() as i64;
                    let days_to_add = 7 - current_weekday;
                    if let Some(next_monday) = current_date.checked_add_signed(chrono::Duration::days(days_to_add)) {
                        final_date_str = format!("{:04}-{:02}-{:02}", next_monday.year(), next_monday.month(), next_monday.day());
                    }
                } else if parsed_date_str.len() == 10 && parsed_date_str.contains('-') {
                    final_date_str = parsed_date_str;
                } else if !parsed_date_str.is_empty() && parsed_date_str != "yyyy-mm-dd" && parsed_date_str != "today" {
                    if let Ok(day_num) = parsed_date_str.parse::<u32>() {
                        final_date_str = format!("{:04}-{:02}-{:02}", db.view_year, db.view_month, day_num);
                    }
                }
                
                db.items.push(SavedTask {
                    id: new_id,
                    title: parsed_task_text,
                    due_date_str: final_date_str,
                    due_time_str: "23:59".to_string(),
                    is_completed: false,
                    priority: final_priority,
                });
                refresh_calendar_grid(&ui, &db);
                save_db(&db);
            }
            refresh_tasks_ui_clone4();
        }
    });

    // 6. Clear Completed Tasks
    let clear_db = db_container.clone();
    let clear_ui_weak = ui.as_weak();
    let refresh_tasks_ui_clone5 = refresh_tasks_ui.clone();
    ui.on_clear_completed_tasks(move || {
        let clear_db = clear_db.clone();
        let ui_weak = clear_ui_weak.clone();
        let refresh = refresh_tasks_ui_clone5.clone();
        tokio::spawn(async move {
            {
                let mut db = clear_db.lock().unwrap();
                db.items.retain(|t| !t.is_completed);
                save_db(&db);
            }
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    if let Ok(db) = clear_db.lock() {
                        refresh_calendar_grid(&ui, &db);
                    }
                    refresh();
                }
            });
        });
    });

    // 7. Todo Filter Changed
    let todo_tab_clone = todo_tab_filter.clone();
    let refresh_tasks_ui_clone6 = refresh_tasks_ui.clone();
    ui.on_todo_filter_changed(move |tab| {
        *todo_tab_clone.lock().unwrap() = tab.to_string();
        refresh_tasks_ui_clone6();
    });

    // 8. Delete Task
    let delete_db = db_container.clone();
    let delete_ui_weak = ui.as_weak();
    let refresh_tasks_ui_clone7 = refresh_tasks_ui.clone();
    ui.on_delete_task(move |id| {
        let delete_db = delete_db.clone();
        let ui_weak = delete_ui_weak.clone();
        let refresh = refresh_tasks_ui_clone7.clone();
        tokio::spawn(async move {
            {
                let mut db = delete_db.lock().unwrap();
                db.items.retain(|t| t.id != id);
                save_db(&db);
            }
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    if let Ok(db) = delete_db.lock() {
                        refresh_calendar_grid(&ui, &db);
                    }
                    refresh();
                }
            });
        });
    });

    // 9. Change Month Offset
    let nav_db = db_container.clone();
    let nav_ui_weak = ui.as_weak();
    let refresh_tasks_ui_clone8 = refresh_tasks_ui.clone();
    ui.on_change_month_offset(move |offset| {
        if let Some(ui) = nav_ui_weak.upgrade() {
            {
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
                refresh_calendar_grid(&ui, &db);
            }
            // Trigger UI refresh to show tasks matching new month view
            refresh_tasks_ui_clone8();
        }
    });
}
