// PURPOSE: Manages Wayland wl_surface.set_input_region for pixel-perfect click passthrough
// INPUTS: display_ptr, surface_ptr, and bounding box coordinates
// USED BY: src/main.rs

use std::ffi::c_void;

extern "C" {
    fn set_wayland_input_region(
        display_ptr: *mut c_void,
        surface_ptr: *mut c_void,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    );
}

pub fn update_input_region(
    display_ptr: *mut c_void,
    surface_ptr: *mut c_void,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    if display_ptr.is_null() || surface_ptr.is_null() {
        return;
    }
    unsafe {
        set_wayland_input_region(display_ptr, surface_ptr, x, y, width, height);
    }
}
