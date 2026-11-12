#ifndef CM_BANNER_H_
#define CM_BANNER_H_

#include <gtk/gtk.h>

static void set_cm_banner_window(GtkWindow* window) {
  // Mutter constrains normal windows using their old size during a move/resize.
  // A panel can stay flush with the screen edge while the banner folds.
  gtk_window_set_type_hint(window, GDK_WINDOW_TYPE_HINT_DOCK);
  gtk_window_set_decorated(window, FALSE);
  gtk_window_set_skip_taskbar_hint(window, TRUE);
  gtk_window_set_skip_pager_hint(window, TRUE);
}

#endif  // CM_BANNER_H_
