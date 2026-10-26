#include <wayland-client.h>
#include <string.h>
#include <stdint.h>

static struct wl_compositor *g_compositor = NULL;

static void registry_handler(void *data, struct wl_registry *registry, uint32_t id, const char *interface, uint32_t version) {
    (void)data;
    (void)version;
    if (strcmp(interface, "wl_compositor") == 0) {
        g_compositor = (struct wl_compositor *)wl_registry_bind(registry, id, &wl_compositor_interface, 1);
    }
}

static void registry_remover(void *data, struct wl_registry *registry, uint32_t id) {
    (void)data;
    (void)registry;
    (void)id;
}

static const struct wl_registry_listener registry_listener = {
    registry_handler,
    registry_remover
};

void set_wayland_input_region(void *display_ptr, void *surface_ptr, int32_t x, int32_t y, int32_t width, int32_t height) {
    if (!display_ptr || !surface_ptr) return;
    struct wl_display *display = (struct wl_display *)display_ptr;
    struct wl_surface *surface = (struct wl_surface *)surface_ptr;

    if (!g_compositor) {
        struct wl_registry *registry = wl_display_get_registry(display);
        if (registry) {
            wl_registry_add_listener(registry, &registry_listener, NULL);
            wl_display_roundtrip(display);
            wl_registry_destroy(registry);
        }
    }

    if (!g_compositor) return;

    struct wl_region *region = wl_compositor_create_region(g_compositor);
    if (!region) return;

    wl_region_add(region, x, y, width, height);
    wl_surface_set_input_region(surface, region);
    wl_surface_commit(surface);
    wl_region_destroy(region);
}
