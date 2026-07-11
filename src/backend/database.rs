use std::fs;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};
use slint::{SharedString, ModelRc, VecModel};
use std::rc::Rc;
use chrono::{Datelike, Local, NaiveDate};

use crate::{TaskItem, CalendarDay, UltimateDynamicIsland};

#[derive(Serialize, Deserialize, Clone)]
pub struct SavedTask {
    pub id: i32,
    pub title: String,
    pub due_date_str: String,
    pub due_time_str: String,
    pub is_completed: bool,
}

pub struct TaskDatabase {
    pub items: Vec<SavedTask>,
    pub view_year: i32,
    pub view_month: u32,
}

pub fn get_db_path() -> PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("rust-island");
    fs::create_dir_all(&path).ok();
    path.push("tasks.json");
    path
}

pub fn load_db() -> TaskDatabase {
    let path = get_db_path();
    let items = if let Ok(data) = fs::read_to_string(&path) {
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        Vec::new()
    };
    let now = Local::now();
    TaskDatabase {
        items,
        view_year: now.year(),
        view_month: now.month(),
    }
}

pub fn save_db(db: &TaskDatabase) {
    let path = get_db_path();
    if let Ok(data) = serde_json::to_string_pretty(&db.items) {
        fs::write(path, data).ok();
    }
}

pub fn update_ui_task_model(ui: &UltimateDynamicIsland, items: &[SavedTask]) {
    let slint_items: Vec<TaskItem> = items.iter().map(|t| TaskItem {
        id: t.id,
        title: SharedString::from(&t.title),
        due_date_str: SharedString::from(&t.due_date_str),
        due_time_str: SharedString::from(&t.due_time_str),
        is_completed: t.is_completed,
    }).collect();
    let model = Rc::new(VecModel::from(slint_items));
    ui.set_active_tasks(ModelRc::from(model));
}

pub fn refresh_calendar_grid(ui: &UltimateDynamicIsland, db: &TaskDatabase) {
    let mut days_vector = Vec::new();
    let month_str = match db.view_month {
        1 => "January", 2 => "February", 3 => "March", 4 => "April",
        5 => "May", 6 => "June", 7 => "July", 8 => "August",
        9 => "September", 10 => "October", 11 => "November", 12 => "December",
        _ => "Unknown",
    };
    let month_year_label = format!("{} {}", month_str, db.view_year);
    ui.set_current_month_year_label(SharedString::from(month_year_label));

    let first_of_month = NaiveDate::from_ymd_opt(db.view_year, db.view_month, 1).unwrap();
    let weekday_offset = first_of_month.weekday().num_days_from_sunday();
    
    for _ in 0..weekday_offset {
        days_vector.push(CalendarDay {
            text: SharedString::from(""),
            has_event: false,
        });
    }
    
    for day_num in 1..=31 {
        let date = NaiveDate::from_ymd_opt(db.view_year, db.view_month, day_num);
        if date.is_some() {
            let target_str = format!("{:04}-{:02}-{:02}", db.view_year, db.view_month, day_num);
            let has_event = db.items.iter().any(|t| t.due_date_str == target_str && !t.is_completed);
            
            days_vector.push(CalendarDay {
                text: SharedString::from(day_num.to_string()),
                has_event,
            });
        }
    }
    
    let model = Rc::new(VecModel::from(days_vector));
    ui.set_calendar_days_grid(ModelRc::from(model));
}
