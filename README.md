# Rust Island 🏝️

An interactive, premium desktop widget inspired by Apple's **Dynamic Island**, custom-built for modern Linux window managers (such as Niri and Hyprland) using **Rust** and **Slint**.

---

## 🌟 Key Features

### 1. Dual Status Compact Dashboard Clock
- **CPU & Memory monitoring**: Displays date and time sandwiched between two circular progress rings: a CPU load ring on the left, and a Memory load ring on the right.
- **Hover tooltips**: Hovering over the CPU chip icon dynamically transitions the clock text into numerical percentages (`CPU: XX% | RAM: YY%`).
- **Interactive diagnostics**: Clicking the CPU chip icon switches directly to the dedicated System diagnostics dashboard.

### 2. Apple-Style Split-Pill Media Player
- **Detached vinyl disc**: When music is playing in the background, the pill splits into a clock pill on the left and a detached spinning record disk on the right.
- **Hover preview**: Hovering over the cover art circle expands it into a preview tab with track metadata.
- **Full control panel**: Clicking the rotating art card opens the expanded Media panel, featuring a progress scrubber timeline, artist metadata card, and glowing media buttons.

### 3. Unified Todo Checklist & Date Picker Modal
- **Horizontal week slider**: Scroll through the calendar week slider directly at the top of the Todo list. Select any day to filter your tasks.
- **Interactive date picker modal**: Click `📅 Custom` to open the full calendar grid as an interactive date picker. Select any grid cell to set that date on the creator form and return to the Todo list instantly.
- **Quick presets**: Schedule tasks quickly using `Today`, `Tomorrow`, or `📅 Custom` preset buttons.

### 4. Zero-Flicker sliding navigation dock
- **State-machine hysteresis**: Pointer tracking bounds keep the navigation panel open when clicking buttons, completely eliminating hover flickering.
- **Color accents**: Stylized navigation icons glow with custom accents tailored to the active tab (Clipboard, Mixer, Media, System Tray, Todo list).

### 5. Hardware & Mixer controls
- Volume and microphone level sliders with animated scroll boundaries.
- Connection control panels for managing and scanning Wi-Fi networks and Bluetooth devices.

---

## 🛠️ Tech Stack
- **Core Engine**: Rust (fast, safe, multithreaded systems execution).
- **GUI & Animations**: Slint (modern UI library for fluid canvas rendering, properties bindings, and spring easing).
- **System Integration**: D-Bus (`zbus`) for dispatching global events (e.g. notifications, keyboard launchers).
- **Metadata Polling**: MPRIS (media controls), sysinfo (hardware metrics).

---

## 🚀 Getting Started

### Prerequisites
Make sure you have the Rust compiler installed on your system. You'll also need standard development libraries for X11/Wayland backends if compiling on Linux.

### Installation
1. **Clone the repository**:
   ```bash
   git clone https://github.com/amal/rust-island.git
   cd rust-island
   ```

2. **Download required Lucide assets**:
   ```bash
   chmod +x download_icons.sh
   ./download_icons.sh
   ```

3. **Run the application**:
   ```bash
   cargo run
   ```

### Global Keyboard Bindings (e.g., in Niri)
You can trigger the application launcher tab by mapping the zbus toggle command to your window manager configuration:
```bash
busctl --user call org.rustisland.Control /org/rustisland/Control org.rustisland.Control ToggleLauncher
```
For example, map this to `Mod+Space` inside your window manager's keyboard bindings configuration.

---

## 📜 License
This project is licensed under the MIT License.
