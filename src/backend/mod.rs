pub mod apps;
pub mod clock;
pub mod database;
pub mod media;
pub mod system_stats;
pub mod audio;
pub mod brightness;
pub mod wifi;
pub mod bluetooth;
pub mod clipboard;
pub mod notifications;
pub mod tray;
pub mod handlers;
pub mod niri;

pub fn log_to_file(msg: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
    let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Some(mut config_path) = dirs::config_dir() {
        config_path.push("rust-island");
        config_path.push("debug.log");
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(config_path)
        {
            let _ = writeln!(file, "[{}] {}", time, msg);
        }
    }
}
