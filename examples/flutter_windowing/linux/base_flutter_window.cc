//
// Created by boyan on 2022/1/27.
//

#include "base_flutter_window.h"

#include <gtk/gtkx.h>

BaseFlutterWindow::BaseFlutterWindow() {
}

void BaseFlutterWindow::Show() {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  gtk_widget_show(GTK_WIDGET(window));
}

void BaseFlutterWindow::BlockButtonPress() {
  if (0 == flutterButtonPressHandler) {
      flutterButtonPressHandler = g_signal_handler_find(
          event_box, G_SIGNAL_MATCH_ID,
          g_signal_lookup("button-press-event", GTK_TYPE_WIDGET), 0, NULL,
          NULL, NULL);
  }

  if (isFlutterButtonPressBlocked) {
      return;
  }
  g_signal_handler_block(event_box, flutterButtonPressHandler);
  isFlutterButtonPressBlocked = true;
}

void BaseFlutterWindow::UnblockButtonPress() {
  if (!isFlutterButtonPressBlocked) {
      return;
  }
  isFlutterButtonPressBlocked = false;
  g_signal_handler_unblock(event_box, flutterButtonPressHandler);
}


void BaseFlutterWindow::Hide() {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  // store bounds
  gint x, y, width, height;
  gtk_window_get_position(GTK_WINDOW(window), &x, &y);
  gtk_window_get_size(GTK_WINDOW(window), &width, &height);
  // size and position lost after hide
  gtk_widget_hide(GTK_WIDGET(window));
  // restore bounds
  gtk_window_move(GTK_WINDOW(window), x, y);
  gtk_window_resize(GTK_WINDOW(window), width, height);
}

bool BaseFlutterWindow::IsHidden() {
  auto window = GetWindow();
  if (!window) {
    return false;
  }
  return !gtk_widget_is_visible(GTK_WIDGET(window));
}

void BaseFlutterWindow::Focus() {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  gtk_window_deiconify(window);
  gtk_window_present(window);
}

void BaseFlutterWindow::SetFullscreen(bool fullscreen) {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  if (fullscreen)
    gtk_window_fullscreen(window);
  else
    gtk_window_unfullscreen(window);
}

void BaseFlutterWindow::SetBounds(double_t x, double_t y, double_t width,
                                  double_t height) {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  gtk_window_move(GTK_WINDOW(window), static_cast<gint>(x),
                  static_cast<gint>(y));
  gtk_window_resize(GTK_WINDOW(window), static_cast<gint>(width),
                    static_cast<gint>(height));
}

FlValue* BaseFlutterWindow::GetBounds() {
  FlValue* result_data = fl_value_new_map();
  auto window = GetWindow();
  if (window) {
    gint x, y, width, height;
    gtk_window_get_position(GTK_WINDOW(window), &x, &y);
    gtk_window_get_size(GTK_WINDOW(window), &width, &height);

    fl_value_set_string_take(result_data, "x", fl_value_new_float(x));
    fl_value_set_string_take(result_data, "y", fl_value_new_float(y));
    fl_value_set_string_take(result_data, "width", fl_value_new_float(width));
    fl_value_set_string_take(result_data, "height", fl_value_new_float(height));
  }
  return result_data;
}

void BaseFlutterWindow::SetTitle(const std::string &title) {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  gtk_window_set_title(GTK_WINDOW(window), title.c_str());
}

void BaseFlutterWindow::Center() {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  gtk_window_set_position(GTK_WINDOW(window), GTK_WIN_POS_CENTER);
}

void BaseFlutterWindow::Close() {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  gtk_window_close(GTK_WINDOW(window));
}

void BaseFlutterWindow::StartDragging() {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  auto screen = gtk_window_get_screen(window);
  auto display = gdk_screen_get_display(screen);
  auto seat = gdk_display_get_default_seat(display);
  auto device = gdk_seat_get_pointer(seat);

  gint root_x, root_y;
  gdk_device_get_position(device, nullptr, &root_x, &root_y);
  guint32 timestamp = (guint32)g_get_monotonic_time();

  gtk_window_begin_move_drag(window, 1, root_x, root_y, timestamp);
  this->isDragging = true;
}

bool BaseFlutterWindow::IsMaximized() {
  auto window = GetWindow();
  if (!window) {
    return false;
  }
  GdkWindowState state = gdk_window_get_state(gtk_widget_get_window(GTK_WIDGET(window)));
  return state & GDK_WINDOW_STATE_MAXIMIZED;
}

bool BaseFlutterWindow::IsMinimized() {
  auto window = GetWindow();
  if (!window) {
    return false;
  }
  GdkWindowState state = gdk_window_get_state(gtk_widget_get_window(GTK_WIDGET(window)));
  return state & GDK_WINDOW_STATE_ICONIFIED;
}

void BaseFlutterWindow::Maximize() {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  gtk_window_maximize(window);
}

int64_t BaseFlutterWindow::GetXID() {
  auto window = GetWindow();
  if (!window) {
    return -1;
  }
  auto gdk_window = gtk_widget_get_window(GTK_WIDGET(window));
  auto xid = GDK_WINDOW_XID(gdk_window);
  fflush(stdout);
  return xid;
}

void BaseFlutterWindow::Unmaximize() {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  gtk_window_unmaximize(window);
}

void BaseFlutterWindow::Minimize() {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  gtk_window_iconify(window);
}

void BaseFlutterWindow::ShowTitlebar(bool show) {
  auto window = GetWindow();
  if (!window) {
    return;
  }
  gtk_window_set_decorated(window, show);
}

void gtk_container_children_callback(GtkWidget *widget, gpointer client_data) {
  GList **children;
  children = (GList **)client_data;
  *children = g_list_prepend(*children, widget);
}

GList *gtk_container_get_all_children(GtkContainer *container) {
  GList *children = NULL;
  gtk_container_forall(container, gtk_container_children_callback, &children);
  return children;
}

void emit_button_release(BaseFlutterWindow *self) {
  auto newEvent = (GdkEventButton *)gdk_event_new(GDK_BUTTON_RELEASE);
  newEvent->x = self->currentPressedEvent.x;
  newEvent->y = self->currentPressedEvent.y;
  newEvent->button = self->currentPressedEvent.button;
  newEvent->type = GDK_BUTTON_RELEASE;
  newEvent->time = g_get_monotonic_time();
  gboolean result;
  g_signal_emit_by_name(self->event_box, "button-release-event", newEvent,
                        &result);
  gdk_event_free((GdkEvent *)newEvent);
}

gboolean onWindowEventAfter(GtkWidget *text_view, GdkEvent *event,
                            BaseFlutterWindow *self) {
  if (event->type == GDK_ENTER_NOTIFY) {
    if (nullptr == self->event_box) {
      return FALSE;
    }
    if (self->isDragging) {
      self->isDragging = false;
      // resolve linux drag issue
      // https://github.com/bitsdojo/bitsdojo_window/blob/e79b2c7d82b95ffc05bd50d19a8f8d322675ad87/bitsdojo_window_linux/linux/window_impl.cpp
      emit_button_release(self);
    }
    if (self->isResizing) {
      self->isResizing = false;
      emit_button_release(self);
    }
  }
  return FALSE;
}

void BaseFlutterWindow::findEventBox(GtkWidget *widget) {
  GList *children;
  GtkWidget *currentChild;
  children = gtk_container_get_all_children(GTK_CONTAINER(widget));
  while (children) {
    currentChild = (GtkWidget *)children->data;
    if (GTK_IS_EVENT_BOX(currentChild)) {
      this->event_box = currentChild;
    }
    children = children->next;
  }
}

void BaseFlutterWindow::Destroy() {
  this->Close();
}

// https://github.com/bitsdojo/bitsdojo_window/blob/e79b2c7d82b95ffc05bd50d19a8f8d322675ad87/bitsdojo_window_linux/linux/window_impl.cpp
gboolean onMousePressHook(GSignalInvocationHint *ihint, guint n_param_values,
                          const GValue *param_values, gpointer data) {
  auto self = reinterpret_cast<BaseFlutterWindow *>(data);

  gpointer instance = g_value_peek_pointer(param_values);

  if (!GTK_IS_EVENT_BOX(instance)) {
    return TRUE;
  }

  GdkEventButton *event =
      (GdkEventButton *)(g_value_get_boxed(param_values + 1));

  // if (self->isOnEdge && !self->isMaximized) {
  //   self->blockButtonPress();
  //   self->isResizing = true;
  //   gtk_window_begin_resize_drag(self->handle, self->currentEdge,
  //   event->button,
  //                                static_cast<gint>(event->x_root),
  //                                static_cast<gint>(event->y_root),
  //                                event->time);
  // }
  memset(&self->currentPressedEvent, 0, sizeof(self->currentPressedEvent));
  memcpy(&self->currentPressedEvent, event, sizeof(self->currentPressedEvent));
  return TRUE;
}

gboolean onMouseReleaseHook(GSignalInvocationHint *ihint,
                                   guint n_param_values,
                                   const GValue *param_values, gpointer data) {
    auto self = reinterpret_cast<BaseFlutterWindow *>(data);

    gpointer instance = g_value_peek_pointer(param_values);

    if (!GTK_IS_EVENT_BOX(instance)) {
        return TRUE;
    }
    // GdkEventButton *event = (GdkEventButton*)(g_value_get_boxed(param_values
    // + 1));
    self->UnblockButtonPress();

    return TRUE;
}

void BaseFlutterWindow::StartResizing(FlValue *args) {
  auto window = GetWindow();
  const gchar *resize_edge =
      fl_value_get_string(fl_value_lookup_string(args, "resizeEdge"));
  auto screen = gtk_window_get_screen(window);
  auto display = gdk_screen_get_display(screen);
  auto seat = gdk_display_get_default_seat(display);
  auto device = gdk_seat_get_pointer(seat);

  gint root_x, root_y;
  gdk_device_get_position(device, nullptr, &root_x, &root_y);
  guint32 timestamp = (guint32)g_get_monotonic_time();

  GdkWindowEdge gdk_window_edge = GDK_WINDOW_EDGE_NORTH_WEST;

  if (strcmp(resize_edge, "topLeft") == 0) {
    gdk_window_edge = GDK_WINDOW_EDGE_NORTH_WEST;
  } else if (strcmp(resize_edge, "top") == 0) {
    gdk_window_edge = GDK_WINDOW_EDGE_NORTH;
  } else if (strcmp(resize_edge, "topRight") == 0) {
    gdk_window_edge = GDK_WINDOW_EDGE_NORTH_EAST;
  } else if (strcmp(resize_edge, "left") == 0) {
    gdk_window_edge = GDK_WINDOW_EDGE_WEST;
  } else if (strcmp(resize_edge, "right") == 0) {
    gdk_window_edge = GDK_WINDOW_EDGE_EAST;
  } else if (strcmp(resize_edge, "bottomLeft") == 0) {
    gdk_window_edge = GDK_WINDOW_EDGE_SOUTH_WEST;
  } else if (strcmp(resize_edge, "bottom") == 0) {
    gdk_window_edge = GDK_WINDOW_EDGE_SOUTH;
  } else if (strcmp(resize_edge, "bottomRight") == 0) {
    gdk_window_edge = GDK_WINDOW_EDGE_SOUTH_EAST;
  }

  this->BlockButtonPress();
  gtk_window_begin_resize_drag(window, gdk_window_edge,
                               this->currentPressedEvent.button, root_x, root_y,
                               timestamp);
  this->isResizing = true;
}


bool BaseFlutterWindow::IsPreventClose() {
  return this->isPreventClose;
}

void BaseFlutterWindow::SetPreventClose(bool setPreventClose) {
  this->isPreventClose = setPreventClose;
}

bool BaseFlutterWindow::IsFullScreen() {
  auto window = GetWindow();
  GdkWindowState state = gdk_window_get_state(gtk_widget_get_window(GTK_WIDGET(window)));
  return state & GDK_WINDOW_STATE_FULLSCREEN;
}
