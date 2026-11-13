#ifndef CM_BANNER_H_
#define CM_BANNER_H_

#include <gtk/gtk.h>
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#endif

static bool is_cm_banner_supported(bool is_connection_manager) {
#ifdef GDK_WINDOWING_X11
  if (GDK_IS_X11_DISPLAY(gdk_display_get_default())) return true;
  // CM must use the backend GTK actually opened, including an explicit
  // GDK_BACKEND override or a failed XWayland connection.
  if (is_connection_manager) return false;

  const gchar* name = g_getenv("DISPLAY");
  if (name == nullptr || *name == '\0') return false;
  // The main window may use Wayland while CM can open XWayland. Probe the
  // display instead of accepting a stale DISPLAY variable as support.
  GdkDisplay* display = gdk_display_open(name);
  if (display == nullptr) return false;
  const bool supported = GDK_IS_X11_DISPLAY(display);
  gdk_display_close(display);
  return supported;
#else
  (void)is_connection_manager;
  return false;
#endif
}

static void set_cm_banner_window(GtkWindow* window) {
  // Mutter disables interactive movement for DOCK windows.
  gtk_window_set_type_hint(window, GDK_WINDOW_TYPE_HINT_UTILITY);
  gtk_window_set_decorated(window, FALSE);
  gtk_window_set_skip_taskbar_hint(window, TRUE);
  gtk_window_set_skip_pager_hint(window, TRUE);
}

static void set_cm_banner_bounds(GtkWindow* window, gint x, gint y,
                                 gint width, gint height) {
  // CM is non-resizable. Update its fixed size hints before moving, otherwise
  // Mutter constrains the new position using the previous expanded size.
  gtk_window_set_default_size(window, width, height);
  gtk_window_resize(window, width, height);
  gtk_container_check_resize(GTK_CONTAINER(window));
  gtk_window_move(window, x, y);
}

#endif  // CM_BANNER_H_
