pub mod apps;
pub mod database;
pub mod media;
pub mod hardware;
pub mod clipboard;
pub mod notifications;
pub mod tray;

pub fn log_to_file(msg: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
    let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open("/home/amal/.config/rust-island/debug.log")
    {
        let _ = writeln!(file, "[{}] {}", time, msg);
    }
}
