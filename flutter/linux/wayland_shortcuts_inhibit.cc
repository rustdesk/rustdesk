// Wayland keyboard shortcuts inhibit implementation
// Uses the zwp_keyboard_shortcuts_inhibit_manager_v1 protocol to request
// the compositor to disable system shortcuts for specific windows.

#include "wayland_shortcuts_inhibit.h"

#if defined(GDK_WINDOWING_WAYLAND) && defined(HAS_KEYBOARD_SHORTCUTS_INHIBIT)

#include <cstring>
#include <gdk/gdkwayland.h>
#include <wayland-client.h>
#include "keyboard-shortcuts-inhibit-unstable-v1-client-protocol.h"

// Data structure to hold inhibitor state for each window
typedef struct {
  struct zwp_keyboard_shortcuts_inhibit_manager_v1* manager;
  struct zwp_keyboard_shortcuts_inhibitor_v1* inhibitor;
} ShortcutsInhibitData;

// Cleanup function for ShortcutsInhibitData
static void shortcuts_inhibit_data_free(gpointer data) {
  ShortcutsInhibitData* inhibit_data = static_cast<ShortcutsInhibitData*>(data);
  if (inhibit_data->inhibitor != NULL) {
    zwp_keyboard_shortcuts_inhibitor_v1_destroy(inhibit_data->inhibitor);
  }
  if (inhibit_data->manager != NULL) {
    zwp_keyboard_shortcuts_inhibit_manager_v1_destroy(inhibit_data->manager);
  }
  g_free(inhibit_data);
}

// Wayland registry handler to find the shortcuts inhibit manager
static void registry_handle_global(void* data, struct wl_registry* registry,
                                   uint32_t name, const char* interface,
                                   uint32_t /*version*/) {
  ShortcutsInhibitData* inhibit_data = static_cast<ShortcutsInhibitData*>(data);
  if (strcmp(interface,
             zwp_keyboard_shortcuts_inhibit_manager_v1_interface.name) == 0) {
    inhibit_data->manager =
        static_cast<zwp_keyboard_shortcuts_inhibit_manager_v1*>(wl_registry_bind(
            registry, name, &zwp_keyboard_shortcuts_inhibit_manager_v1_interface,
            1));
  }
}

static void registry_handle_global_remove(void* /*data*/, struct wl_registry* /*registry*/,
                                          uint32_t /*name*/) {
  // Not needed for this use case
}

static const struct wl_registry_listener registry_listener = {
    registry_handle_global,
    registry_handle_global_remove,
};

// Inhibitor event handlers
static void inhibitor_active(void* /*data*/,
                             struct zwp_keyboard_shortcuts_inhibitor_v1* /*inhibitor*/) {
  // Inhibitor is now active, shortcuts are being captured
}

static void inhibitor_inactive(void* /*data*/,
                               struct zwp_keyboard_shortcuts_inhibitor_v1* /*inhibitor*/) {
  // Inhibitor is now inactive, shortcuts restored to compositor
}

static const struct zwp_keyboard_shortcuts_inhibitor_v1_listener inhibitor_listener = {
    inhibitor_active,
    inhibitor_inactive,
};

// Forward declaration
static void uninhibit_keyboard_shortcuts(GtkWindow* window);

// Inhibit keyboard shortcuts on Wayland for a specific window
static void inhibit_keyboard_shortcuts(GtkWindow* window) {
  GdkDisplay* display = gtk_widget_get_display(GTK_WIDGET(window));
  if (!GDK_IS_WAYLAND_DISPLAY(display)) {
    return;
  }

  // Check if already inhibited for this window
  if (g_object_get_data(G_OBJECT(window), "shortcuts-inhibit-data") != NULL) {
    return;
  }

  ShortcutsInhibitData* inhibit_data = g_new0(ShortcutsInhibitData, 1);

  struct wl_display* wl_display = gdk_wayland_display_get_wl_display(display);
  if (wl_display == NULL) {
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  struct wl_registry* registry = wl_display_get_registry(wl_display);
  if (registry == NULL) {
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  wl_registry_add_listener(registry, &registry_listener, inhibit_data);
  wl_display_roundtrip(wl_display);

  if (inhibit_data->manager == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  GdkWindow* gdk_window = gtk_widget_get_window(GTK_WIDGET(window));
  if (gdk_window == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  struct wl_surface* surface = gdk_wayland_window_get_wl_surface(gdk_window);
  if (surface == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  GdkSeat* gdk_seat = gdk_display_get_default_seat(display);
  if (gdk_seat == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  struct wl_seat* seat = gdk_wayland_seat_get_wl_seat(gdk_seat);
  if (seat == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  inhibit_data->inhibitor =
      zwp_keyboard_shortcuts_inhibit_manager_v1_inhibit_shortcuts(
          inhibit_data->manager, surface, seat);

  if (inhibit_data->inhibitor == NULL) {
    wl_registry_destroy(registry);
    shortcuts_inhibit_data_free(inhibit_data);
    return;
  }

  // Add listener to monitor active/inactive state
  zwp_keyboard_shortcuts_inhibitor_v1_add_listener(
      inhibit_data->inhibitor, &inhibitor_listener, window);

  wl_display_roundtrip(wl_display);
  wl_registry_destroy(registry);

  // Associate the inhibit data with the window for cleanup on destroy
  g_object_set_data_full(G_OBJECT(window), "shortcuts-inhibit-data",
                         inhibit_data, shortcuts_inhibit_data_free);
}

// Remove keyboard shortcuts inhibitor from a window
static void uninhibit_keyboard_shortcuts(GtkWindow* window) {
  ShortcutsInhibitData* inhibit_data = static_cast<ShortcutsInhibitData*>(
      g_object_get_data(G_OBJECT(window), "shortcuts-inhibit-data"));

  if (inhibit_data == NULL) {
    return;
  }

  // This will trigger shortcuts_inhibit_data_free via g_object_set_data
  g_object_set_data(G_OBJECT(window), "shortcuts-inhibit-data", NULL);
}

// Focus event handlers for dynamic inhibitor management
static gboolean on_window_focus_in(GtkWidget* widget, GdkEventFocus* /*event*/, gpointer /*user_data*/) {
  if (GTK_IS_WINDOW(widget)) {
    inhibit_keyboard_shortcuts(GTK_WINDOW(widget));
  }
  return FALSE;  // Continue event propagation
}

static gboolean on_window_focus_out(GtkWidget* widget, GdkEventFocus* /*event*/, gpointer /*user_data*/) {
  if (GTK_IS_WINDOW(widget)) {
    uninhibit_keyboard_shortcuts(GTK_WINDOW(widget));
  }
  return FALSE;  // Continue event propagation
}

// Key for marking window as having focus handlers connected
static const char* const kFocusHandlersConnectedKey = "shortcuts-inhibit-focus-handlers-connected";
// Key for marking window as having a pending realize handler
static const char* const kRealizeHandlerConnectedKey = "shortcuts-inhibit-realize-handler-connected";

// Callback when window is realized (mapped to screen)
// Sets up focus-based inhibitor management
static void on_window_realize(GtkWidget* widget, gpointer /*user_data*/) {
  if (GTK_IS_WINDOW(widget)) {
    // Check if focus handlers are already connected to avoid duplicates
    if (g_object_get_data(G_OBJECT(widget), kFocusHandlersConnectedKey) != NULL) {
      return;
    }

    // Connect focus events for dynamic inhibitor management
    g_signal_connect(widget, "focus-in-event",
                     G_CALLBACK(on_window_focus_in), NULL);
    g_signal_connect(widget, "focus-out-event",
                     G_CALLBACK(on_window_focus_out), NULL);

    // Mark as connected to prevent duplicate connections
    g_object_set_data(G_OBJECT(widget), kFocusHandlersConnectedKey, GINT_TO_POINTER(1));

    // If window already has focus, create inhibitor now
    if (gtk_window_has_toplevel_focus(GTK_WINDOW(widget))) {
      inhibit_keyboard_shortcuts(GTK_WINDOW(widget));
    }
  }
}

// Public API: Initialize shortcuts inhibit for a sub-window
void wayland_shortcuts_inhibit_init_for_subwindow(void* view) {
  GtkWidget* widget = GTK_WIDGET(view);
  GtkWidget* toplevel = gtk_widget_get_toplevel(widget);

  if (toplevel != NULL && GTK_IS_WINDOW(toplevel)) {
    // Check if already initialized to avoid duplicate realize handlers
    if (g_object_get_data(G_OBJECT(toplevel), kFocusHandlersConnectedKey) != NULL ||
        g_object_get_data(G_OBJECT(toplevel), kRealizeHandlerConnectedKey) != NULL) {
      return;
    }

    if (gtk_widget_get_realized(toplevel)) {
      // Window is already realized, set up focus handlers now
      on_window_realize(toplevel, NULL);
    } else {
      // Mark realize handler as connected to prevent duplicate connections
      // if called again before window is realized
      g_object_set_data(G_OBJECT(toplevel), kRealizeHandlerConnectedKey, GINT_TO_POINTER(1));
      // Wait for window to be realized
      g_signal_connect(toplevel, "realize",
                       G_CALLBACK(on_window_realize), NULL);
    }
  }
}

#endif  // defined(GDK_WINDOWING_WAYLAND) && defined(HAS_KEYBOARD_SHORTCUTS_INHIBIT)
