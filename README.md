# Rust Island 🏝️

An interactive, premium desktop widget inspired by Apple's **Dynamic Island**, custom-built for modern Linux window managers (such as Niri and Hyprland) using **Rust** and **Slint**.

---

## 🌟 Key Features

### 1. Dual Status Compact Dashboard Clock
- **CPU & Memory monitoring**: Displays date and time sandwiched between two circular progress rings: a CPU load ring on the left, and a Memory load ring on the right.
- **Hover tooltips**: Hovering over the CPU chip icon dynamically transitions the clock text into numerical percentages (`CPU: XX% | RAM: YY%`) with fluid animation.
- **Interactive diagnostics**: Clicking the CPU chip icon switches directly to the dedicated System diagnostics dashboard.

### 2. Apple-Style Split-Pill Media Player
- **Detached vinyl disc**: When music is playing in the background, the pill splits into a clock pill on the left and a detached spinning record disk on the right.
- **Hover preview**: Hovering over the cover art circle expands it into a preview tab with track metadata.
- **Full control panel**: Clicking the rotating art card opens the expanded Media panel, featuring a progress scrubber timeline, volume control, artist metadata card, and glowing media buttons.

### 3. Unified Todo Checklist & Date Picker Modal
- **Horizontal week slider**: Scroll through the calendar week slider directly at the top of the Todo list. Select any day to filter your tasks.
- **Interactive date picker modal**: Click `📅 Custom` to open the full calendar grid as an interactive date picker. Select any grid cell to set that date on the creator form and return to the Todo list instantly.
- **Task Presets & Custom Alerts**: Schedule tasks quickly using `Today`, `Tomorrow`, or custom presets, and toggle audio alerts ("Chime") for due tasks.

### 4. Zero-Flicker Sliding Navigation Dock
- **State-machine hysteresis**: Pointer tracking bounds keep the navigation panel open when clicking buttons, completely eliminating hover flickering.
- **Color accents**: Stylized navigation icons glow with custom accents tailored to the active tab (Clipboard, Mixer, Media, System Tray, Todo list).

### 5. Hardware & Mixer Controls
- **Scrolling Audio Mixer**: Control output volume and input microphone levels. The interface includes scrollable audio source/sink selectors to easily toggle active audio devices.
- **Connectivity Panels**: Scan, connect, and disconnect Wi-Fi networks and Bluetooth devices in dedicated, clean control tabs.

---

## 🛠️ Tech Stack
- **Core Engine**: Rust (fast, safe, multithreaded systems execution).
- **GUI & Animations**: Slint (modern UI library for fluid canvas rendering, properties bindings, and spring easing).
- **System Integration**: D-Bus (`zbus`) for dispatching global events (e.g. notifications, keyboard launchers).
- **Metadata Polling**: MPRIS (media controls), sysinfo (hardware metrics), PulseAudio/PipeWire (audio), NetworkManager/ConnMan (Wi-Fi).

---

## 🚀 Installation & Setup

### Prerequisites
Make sure you have the Rust toolchain installed. You'll also need standard development libraries for X11/Wayland backends if compiling on Linux:
- `dbus` development headers
- `fontconfig` and `freetype`
- `pkg-config`

On Debian/Ubuntu:
```bash
sudo apt install -y pkg-config libdbus-1-dev libfontconfig1-dev libx11-dev libwayland-dev
```

On Arch Linux:
```bash
sudo pacman -S --needed pkgconf dbus fontconfig wayland
```

### Automatic Installation

We provide an automated installer script that downloads required icons, compiles the application in release mode, installs the binary and `.desktop` launcher, and optionally sets up a systemd user daemon.

To run the installer:
```bash
chmod +x install.sh
./install.sh
```

---

## 🌀 Window Manager Configuration

### Niri Window Manager Specific Setup

To make the Dynamic Island floating, borderless, and auto-focused on launch, add the following to your `~/.config/niri/config.kdl` file:

```kdl
// Window Rules to float, remove borders, and center the dynamic island
window-rule {
    match-app-id "rust-island"
    
    open-floating true
    
    border { off; }
    focus-ring { off; }
}
```

### Global Keyboard Bindings (e.g., in Niri)
You can bind the D-Bus trigger to a key combination (e.g., `Mod+Space`) to toggle the launcher and focus the input immediately. Add this bind to your `config.kdl`:

```kdl
binds {
    Mod+Space { spawn "busctl" "--user" "call" "org.rustisland.Control" "/Control" "org.rustisland.Control" "ToggleLauncher"; }
}
```

---

## 📜 License
This project is licensed under the MIT License.
