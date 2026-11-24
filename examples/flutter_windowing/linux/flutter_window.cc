//
// Created by yangbin on 2022/1/11.
//

#include "flutter_window.h"

#include <iostream>
#include <cairo/cairo.h>

#include "include/desktop_multi_window/desktop_multi_window_plugin.h"
#include "desktop_multi_window_plugin_internal.h"

extern void fl_register_plugins(FlPluginRegistry *registry);

bool rustdesk_is_subwindow = false;

GtkWidget *find_gl_area(GtkWidget *widget);

namespace
{

  WindowCreatedCallback _g_window_created_callback = nullptr;

}

gboolean DrawCallback(GtkWidget* widget, cairo_t* cr, gpointer data) {
  cairo_save(cr);
  cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 0.0);
  cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
  cairo_paint(cr);
  cairo_restore(cr);
  return FALSE;
}

FlutterWindow::FlutterWindow(
    int64_t id,
    const std::string &args,
    const std::shared_ptr<FlutterWindowCallback> &callback) : callback_(callback), id_(id)
{
  window_ = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  gtk_window_set_decorated(GTK_WINDOW(window_), FALSE);
  gtk_window_set_default_size(GTK_WINDOW(window_), 1280, 720);
  gtk_window_set_position(GTK_WINDOW(window_), GTK_WIN_POS_CENTER);
  // try setting icon for rustdesk, which uses the system cache
  // mainly for the icon in appimage.
  GtkIconTheme* theme = gtk_icon_theme_get_default();
  gint icons[4] = {256, 128, 64, 32};
  for (int i = 0; i < 4; i++) {
    GdkPixbuf* icon = gtk_icon_theme_load_icon(theme, "rustdesk", icons[i], GTK_ICON_LOOKUP_NO_SVG, NULL);
    if (icon != nullptr) {
      gtk_window_set_icon(GTK_WINDOW(window_), icon);
    }
  }

  // set gtk header bar
  // fix for the frame of sub window exists after hide header bar on wayland
  const char *display_server = getenv("WAYLAND_DISPLAY");
  if (display_server) {
    GtkHeaderBar *header_bar = GTK_HEADER_BAR(gtk_header_bar_new());
    gtk_widget_show(GTK_WIDGET(header_bar));
    gtk_header_bar_set_title(header_bar, "");
    gtk_header_bar_set_show_close_button(header_bar, TRUE);
    gtk_window_set_titlebar(GTK_WINDOW(window_), GTK_WIDGET(header_bar));
  }
  g_autoptr(FlDartProject)
      project = fl_dart_project_new();
  const char *entrypoint_args[] = {"multi_window", g_strdup_printf("%ld", id_), args.c_str(), nullptr};
  fl_dart_project_set_dart_entrypoint_arguments(project, const_cast<char **>(entrypoint_args));

  FlView* fl_view = fl_view_new(project);
  gtk_container_add(GTK_CONTAINER(window_), GTK_WIDGET(fl_view));

  // https://github.com/flutter/flutter/issues/152154
  // Remove this workaround when flutter version is updated.
  GtkWidget *gl_area = find_gl_area(GTK_WIDGET(fl_view));
  if (gl_area != NULL) {
    gtk_gl_area_set_has_alpha(GTK_GL_AREA(gl_area), TRUE);
  }

  GdkScreen* screen_ = gtk_window_get_screen(GTK_WINDOW(window_));
  GdkVisual *visual_;
  gtk_widget_set_app_paintable(GTK_WIDGET(window_), TRUE);
  visual_ = gdk_screen_get_rgba_visual(screen_);
  if (visual_ != NULL && gdk_screen_is_composited(screen_)) {
    gtk_widget_set_visual(GTK_WIDGET(window_), visual_);
  }

  if (_g_window_created_callback)
  {
    _g_window_created_callback(FL_PLUGIN_REGISTRY(fl_view));
  }
  // indicate to plugin injections using extern
  rustdesk_is_subwindow = true;
  fl_register_plugins(FL_PLUGIN_REGISTRY(fl_view));
  g_autoptr(FlPluginRegistrar)
      desktop_multi_window_registrar =
          fl_plugin_registry_get_registrar_for_plugin(FL_PLUGIN_REGISTRY(fl_view), "DesktopMultiWindowPlugin");
  desktop_multi_window_plugin_register_with_registrar_internal(desktop_multi_window_registrar);

  window_channel_ = WindowChannel::RegisterWithRegistrar(desktop_multi_window_registrar, id_);

  // Refer to: https://github.com/alexmercerind/flutter_acrylic
  //
  // Set a transparent background for RustDesk.
  // Because we need to control the whole remote screen better, especially for the edge part of the remote screen. We gives a padding for the flutter view, and set a transparent border surround with the flutter view.
  gtk_widget_set_app_paintable(GTK_WIDGET(window_), TRUE);
  auto screen = gdk_screen_get_default();
  auto visual = gdk_screen_get_rgba_visual(screen);
  if (visual != NULL && gdk_screen_is_composited(screen)) {
    gtk_widget_set_visual(GTK_WIDGET(window_), visual);
  }
  g_signal_connect(G_OBJECT(window_), "draw", G_CALLBACK(DrawCallback), this);

  gtk_widget_show(GTK_WIDGET(window_));
  gtk_widget_show(GTK_WIDGET(fl_view));

  // Disconnect all delete-event handlers first in flutter 3.10.1, which causes delete_event not working.
  // Issues from flutter/engine: https://github.com/flutter/engine/pull/40033
  guint handler_id = g_signal_handler_find(GTK_WIDGET(window_), G_SIGNAL_MATCH_DATA, 0, 0, NULL, NULL, fl_view);
  if (handler_id > 0) {
    g_signal_handler_disconnect(GTK_WIDGET(window_), handler_id);
  }

  g_signal_connect(window_, "delete-event", G_CALLBACK(onWindowClose), this);
  g_signal_connect(window_, "window-state-event",
                   G_CALLBACK(onWindowStateChange), this);
  g_signal_connect(window_, "focus-in-event",
                   G_CALLBACK(onWindowFocus), this);
  g_signal_connect(window_, "focus-out-event",
                   G_CALLBACK(onWindowBlur), this);
  g_signal_connect(window_, "configure-event",
                   G_CALLBACK(onWindowMove), this);
  g_signal_connect(window_, "check-resize",
                   G_CALLBACK(onWindowResize), this);
  g_signal_connect(window_, "show", G_CALLBACK(onWindowShow),
                   this);
  g_signal_connect(window_, "hide", G_CALLBACK(onWindowHide),
                   this);
  // enhance drag
  g_signal_connect(window_, "event-after", G_CALLBACK(onWindowEventAfter),
                   this);
  this->findEventBox(GTK_WIDGET(fl_view));
  this->pressedEmissionHook = g_signal_add_emission_hook(
      g_signal_lookup("button-press-event", GTK_TYPE_WIDGET), 0,
      onMousePressHook, this, NULL);
  g_signal_add_emission_hook(
            g_signal_lookup("button-release-event", GTK_TYPE_WIDGET), 0,
            onMouseReleaseHook, this, NULL);

  gtk_widget_grab_focus(GTK_WIDGET(fl_view));
  gtk_widget_hide(GTK_WIDGET(window_));
}

WindowChannel *FlutterWindow::GetWindowChannel()
{
  return window_channel_.get();
}

int64_t FlutterWindow::GetId()
{
  return this->id_;
}

FlutterWindow::~FlutterWindow()
{
  g_signal_remove_emission_hook(g_signal_lookup("button-press-event", GTK_TYPE_WIDGET), this->pressedEmissionHook);
  if (this->window_)
  {
    gtk_widget_destroy(this->window_);
    this->window_ = nullptr;
  }
  if (this->window_channel_ != nullptr)
  {
    this->window_channel_.reset();
    this->window_channel_ = nullptr;
  }
};

void desktop_multi_window_plugin_set_window_created_callback(WindowCreatedCallback callback)
{
  _g_window_created_callback = callback;
}

void _emitEvent(const char *event_name, FlutterWindow *self)
{
  g_autoptr(FlValue) result_data = fl_value_new_map();
  fl_value_set_string_take(result_data, "eventName",
                           fl_value_new_string(event_name));
  fl_value_set_string_take(result_data, "windowId",
                           fl_value_new_int(self->GetId()));
  self->GetWindowChannel()->InvokeMethodSelfVoid("onEvent", result_data);
}

gboolean onWindowClose(GtkWidget *widget, GdkEvent *, gpointer arg)
{
  auto *self = static_cast<FlutterWindow *>(arg);
  // destory hook
  if (!self->isPreventClose)
  {
    if (auto channel = self->GetWindowChannel())
    {
      auto args = fl_value_new_map();
      channel->InvokeMethodSelfVoid("onDestroy", args);
    }
    if (auto callback = self->callback_.lock())
    {
      callback->OnWindowClose(self->id_);
      callback->OnWindowDestroy(self->id_);
    }
  }
  else
  {
    _emitEvent("close", self);
  }
  return self->isPreventClose;
}

gboolean onWindowFocus(GtkWidget *widget, GdkEvent *event, gpointer data)
{
  auto *self = static_cast<FlutterWindow *>(data);
  _emitEvent("focus", self);
  return false;
}

gboolean onWindowBlur(GtkWidget *widget, GdkEvent *event, gpointer data)
{
  auto *self = static_cast<FlutterWindow *>(data);
  _emitEvent("blur", self);
  return false;
}

gboolean onWindowResize(GtkWidget *widget, gpointer data)
{
  auto *self = static_cast<FlutterWindow *>(data);
  _emitEvent("resize", self);
  _emitEvent("resized", self);
  return false;
}


gboolean onWindowShow(GtkWidget *widget, gpointer data)
{
  auto *self = static_cast<FlutterWindow *>(data);
  _emitEvent("show", self);
  return false;
}


gboolean onWindowHide(GtkWidget *widget, gpointer data)
{
  auto *self = static_cast<FlutterWindow *>(data);
  _emitEvent("hide", self);
  return false;
}

gboolean onWindowMove(GtkWidget *widget, GdkEvent *event, gpointer data)
{
  auto *self = static_cast<FlutterWindow *>(data);
  _emitEvent("move", self);
  _emitEvent("moved", self);
  return false;
}

gboolean onWindowStateChange(GtkWidget *widget,
                             GdkEventWindowState *event,
                             gpointer arg)
{
  auto *self = static_cast<FlutterWindow *>(arg);
  if (event->changed_mask & GDK_WINDOW_STATE_MAXIMIZED)
  {
    if (event->new_window_state & GDK_WINDOW_STATE_MAXIMIZED) {
      _emitEvent("maximize", self);
    } else {
      _emitEvent("unmaximize", self);
    }
  }
  if (event->changed_mask & GDK_WINDOW_STATE_ICONIFIED) {
    if (event->new_window_state & GDK_WINDOW_STATE_ICONIFIED) {
      _emitEvent("minimize", self);
    } else {
      _emitEvent("restore", self);
    }
  }

  if (event->changed_mask & GDK_WINDOW_STATE_FULLSCREEN) {
    if (event->new_window_state & GDK_WINDOW_STATE_FULLSCREEN) {
      _emitEvent("enter-full-screen", self);
    } else {
      _emitEvent("leave-full-screen", self);
    }
  }
  return false;
}

GtkWidget *find_gl_area(GtkWidget *widget)
{
  if (GTK_IS_GL_AREA(widget)) {
    return widget;
  }

  if (GTK_IS_CONTAINER(widget)) {
    GList *children = gtk_container_get_children(GTK_CONTAINER(widget));
    for (GList *iter = children; iter != NULL; iter = g_list_next(iter)) {
      GtkWidget *child = GTK_WIDGET(iter->data);
      GtkWidget *gl_area = find_gl_area(child);
      if (gl_area != NULL) {
        g_list_free(children);
        return gl_area;
      }
    }
    g_list_free(children);
  }

  return NULL;
}
