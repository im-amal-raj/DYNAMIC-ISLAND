use slint::ComponentHandle;
use crate::{UltimateDynamicIsland, IslandSection};

pub fn setup(ui: &UltimateDynamicIsland) {
    // 1. Set master output volume
    ui.on_set_volume(|vol| {
        let _ = std::process::Command::new("wpctl")
            .arg("set-volume")
            .arg("@DEFAULT_AUDIO_SINK@")
            .arg(format!("{}%", vol as u32))
            .spawn();
    });

    // 2. Set microphone level
    ui.on_set_mic_volume(|vol| {
        let _ = std::process::Command::new("wpctl")
            .arg("set-volume")
            .arg("@DEFAULT_AUDIO_SOURCE@")
            .arg(format!("{}%", vol as u32))
            .spawn();
    });

    // 3. Set brightness level
    ui.on_set_brightness(|val| {
        let _ = std::process::Command::new("brightnessctl")
            .arg("set")
            .arg(format!("{}%", val as u32))
            .spawn();
    });
    
    // 4. Tab cycling handler
    let cycle_ui_weak = ui.as_weak();
    ui.on_cycle_tab(move |offset| {
        if let Some(ui) = cycle_ui_weak.upgrade() {
            let current = ui.get_active_section();
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
    
    // 5. Connect to Wi-Fi SSID
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

    // 6. Disconnect from Wi-Fi
    ui.on_disconnect_wifi(|| {
        let _ = std::process::Command::new("nmcli").arg("dev").arg("disconnect").spawn();
    });

    // 7. Rescan Wi-Fi
    ui.on_scan_wifi(|| {
        let _ = std::process::Command::new("nmcli").arg("dev").arg("wifi").arg("rescan").spawn();
    });

    // 8. Toggle Wi-Fi state
    ui.on_toggle_wifi(|enabled| {
        let state = if enabled { "on" } else { "off" };
        let _ = std::process::Command::new("nmcli").arg("radio").arg("wifi").arg(state).spawn();
    });

    // 9. Rescan Bluetooth
    ui.on_scan_bluetooth(|| {
        let _ = std::process::Command::new("bluetoothctl").arg("scan").arg("on").spawn();
    });

    // 10. Toggle Bluetooth power
    ui.on_toggle_bluetooth(|enabled| {
        let state = if enabled { "power on" } else { "power off" };
        let _ = std::process::Command::new("bluetoothctl").arg(state).spawn();
    });

    // 11. Connect Bluetooth Device
    ui.on_connect_bluetooth(|mac| {
        let _ = std::process::Command::new("bluetoothctl").arg("connect").arg(mac.as_str()).spawn();
    });

    // 12. Disconnect Bluetooth Device
    ui.on_disconnect_bluetooth(|mac| {
        let _ = std::process::Command::new("bluetoothctl").arg("disconnect").arg(mac.as_str()).spawn();
    });

    // 13. Set Default Audio Device
    ui.on_set_default_audio(|id| {
        let _ = std::process::Command::new("wpctl").arg("set-default").arg(id.as_str()).spawn();
    });
}
