fn main() {
    cc::Build::new()
        .file("src/backend/wayland_input_region.c")
        .compile("wayland_input_region");
    println!("cargo:rustc-link-lib=wayland-client");

    slint_build::compile("src/island.slint").unwrap();
}
