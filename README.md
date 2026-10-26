# Rust Island 🏝️

A native, high-performance **Dynamic Island** for modern Wayland compositors, engineered in **Rust** and **Slint**. 

Designed as an uncompromised, zero-latency desktop interface following Apple Human Interface Guidelines (HIG): fluid spring physics, continuous squircle curvature, liquid glass materials, and dynamic Wayland click-passthrough.

---

## 🏛️ Architecture & AI Portability Guide

If you are an AI coding agent or systems engineer porting or integrating Rust Island into a new Wayland compositor (e.g. Umbriel, Niri, Hyprland, Sway, River, KWin), this section outlines the system contract.

```
                              RUST ISLAND ARCHITECTURE
                                         │
        ┌────────────────────────────────┴────────────────────────────────┐
        ▼                                                                 ▼
[ SLINT UI RUNTIME ]                                            [ TOKIO SYSTEMS BACKEND ]
  - Declarative reactive canvas                                   - Multithreaded async polling loops
  - Apple HIG spring physics & materials                          - D-Bus (MPRIS, StatusNotifierItem, Notifications)
  - Modular panel tree (src/ui/panels/)                           - Zero-overhead sysfs sensors (<0.1ms)
        │                                                                 │
        └────────────────────────────────┬────────────────────────────────┘
                                         ▼
                            [ WAYLAND RUNTIME LAYER ]
                     - Floating xdg_toplevel window
                     - Dynamic input region masking (wl_surface.set_input_region)
                     - Zero-click-blocking background passthrough
```

---

## 🧩 The Core Wayland Problem & Input Region Solution

Standard desktop widgets either use `wlr-layer-shell` (which reserves screen space or lacks flexible interactive popups) or standard `xdg_toplevel` floating windows.

### The Invisible Hit-Box Trap
When expanded into panels (Todo, Controls, Clipboard, Media), the island requires a **$420 \times 385\text{px}$** canvas. When collapsed into the compact dock pill, the visible capsule is only **$220 \times 42\text{px}$**.
* Without input masking, a $420\times385\text{px}$ transparent window captures all mouse clicks, completely blocking users from clicking browser tabs or editor buttons behind the transparent area.
* Constantly destroying and recreating Wayland buffers causes graphical stuttering and compositor desync.

### The Solution: Dynamic Inode Input Masking
Rust Island stays fixed at $420\times385\text{px}$ but dynamically updates the compositor's input hit-box via **`wl_surface.set_input_region`** in [`src/backend/wayland_input_region.c`](src/backend/wayland_input_region.c):
1. **Compact Mode**: Shrink-wraps the input region strictly to the $220\times42\text{px}$ capsule. All transparent empty space passes clicks directly to underlying applications.
2. **Expanded Mode**: Expands the input region to cover the active panel geometry ($380\text{px} - 420\text{px}$ width).

---

## ⚙️ Compositor Porting Configuration

To port Rust Island to your Wayland window manager, configure a window rule to float the window, center it at the top edge, and remove decorations:

### 1. Umbriel / Noctalia (`~/.config/umbriel/config.toml`)
```toml
[[window_rule]]
match.app_id = "^rust-island$"
default_floating = true
default_floating_size_px = { width = 420, height = 385 }
default_position = { x = 0, y = 8, anchor = "top" }
border_width = 0
shadow = false

[keybinds]
"Mod+Space" = { action = "spawn:/home/amal/.local/bin/rust-island", repeat = false }
```

### 2. Niri (`~/.config/niri/config.kdl`)
```kdl
window-rule {
    match-app-id "rust-island"
    open-floating true
    default-floating-size { width 420; height 385; }
    border { off; }
    focus-ring { off; }
    shadow { off; }
}

binds {
    Mod+Space { spawn "rust-island"; }
}
```

### 3. Hyprland (`~/.config/hypr/hyprland.conf`)
```ini
windowrulev2 = float, class:^(rust-island)$
windowrulev2 = noborder, class:^(rust-island)$
windowrulev2 = noshadow, class:^(rust-island)$
windowrulev2 = pin, class:^(rust-island)$
windowrulev2 = move 50% 8, class:^(rust-island)$
windowrulev2 = size 420 385, class:^(rust-island)$

bind = SUPER, SPACE, exec, rust-island
```

### 4. Sway / wlroots (`~/.config/sway/config`)
```ini
for_window [app_id="rust-island"] {
    floating enable
    border none
    move position center 8 px
}

bindsym $mod+space exec rust-island
```

---

## 🔌 Backend Subsystems & Protocols

| Resource | Protocol / Mechanism | Implementation File | Performance Characteristic |
| :--- | :--- | :--- | :--- |
| **Media Player** | MPRIS D-Bus (`org.mpris.MediaPlayer2.*`) | [`src/backend/media.rs`](src/backend/media.rs) | Event-driven playback state & album art |
| **Wi-Fi** | NetworkManager (`nmcli --rescan no`) | [`src/backend/wifi.rs`](src/backend/wifi.rs) | Sub-30ms instant cached kernel probe |
| **Bluetooth** | BlueZ (`bluetoothctl devices Connected`) | [`src/backend/bluetooth.rs`](src/backend/bluetooth.rs) | Batch set lookup; 0 subshell loops |
| **Display Backlight** | Linux Sysfs (`/sys/class/backlight/`) | [`src/backend/brightness.rs`](src/backend/brightness.rs) | Zero-overhead file read (<0.1ms) |
| **Audio & Sinks** | PipeWire / PulseAudio (`wpctl` / `pactl`) | [`src/backend/audio.rs`](src/backend/audio.rs) | Volume deltas with device switching |
| **Clipboard** | `cliphist` integration via stdio | [`src/backend/clipboard.rs`](src/backend/clipboard.rs) | Dual text/image snippets with search |
| **Notifications** | D-Bus Notification Server | [`src/backend/notifications.rs`](src/backend/notifications.rs) | Native banner peek with auto-dismiss |
| **App Launcher** | D-Bus Single-Instance (`ToggleLauncher`) | [`src/main.rs`](src/main.rs) | Instant toggle without re-initializing |

---

## 🎨 Apple HIG Design Tokens

All UI styles derive from semantic tokens in [`src/ui/theme.slint`](src/ui/theme.slint):
- **Motion Physics**: Expressive spring easing ($T_0 = 0.35\text{s}, \zeta = 0.85$, `cubic-bezier(0.16, 1.0, 0.3, 1.0)`).
- **Controls**: Apple Control Center white liquid glass sliders (`@linear-gradient(90deg, rgba(255,255,255,0.92) 0%, rgba(255,255,255,1.0) 100%)`) with dynamic icon inversion when fill $>15\%$.
- **Surfaces**: 4-tier dark charcoal hierarchy (`rgba(28, 28, 30)` through `rgba(58, 58, 60)`) with 1px directional edge highlights.
- **Auto-Collapse**: Expansions automatically collapse to compact mode after **20 seconds** of inactivity, safely reset by any touch, drag, or typing interaction.

---

## 🛠️ Build & Run

```bash
# Check dependencies and compilation
cargo check

# Compile optimized native binary
cargo build --release

# Install binary atomically
install -m 755 target/release/rust-island ~/.local/bin/rust-island
```
