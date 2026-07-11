use slint::ComponentHandle;
use std::time::Duration;
use sysinfo::{System, CpuRefreshKind, RefreshKind, MemoryRefreshKind};
use crate::UltimateDynamicIsland;

pub fn setup_hardware(ui: &UltimateDynamicIsland) {
    let cpu_ui_handle = ui.as_weak();
    tokio::spawn(async move {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
        );
        
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            sys.refresh_all();
            let global_cpu = sys.global_cpu_info().cpu_usage();
            let used_mem = sys.used_memory();
            let total_mem = sys.total_memory();
            let mem_usage = if total_mem > 0 { used_mem as f32 / total_mem as f32 } else { 0.0 };

            let _ = slint::invoke_from_event_loop({
                let ui_handle = cpu_ui_handle.clone();
                move || {
                    if let Some(ui) = ui_handle.upgrade() {
                        ui.set_cpu_usage(global_cpu / 100.0);
                        ui.set_memory_usage(mem_usage);
                    }
                }
            });
        }
    });
}
