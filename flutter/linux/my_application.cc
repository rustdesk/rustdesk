#include "my_application.h"

#include "bump_mouse.h"

#include <flutter_linux/flutter_linux.h>
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#endif
#if defined(GDK_WINDOWING_WAYLAND) && defined(HAS_KEYBOARD_SHORTCUTS_INHIBIT)
#include "wayland_shortcuts_inhibit.h"
#endif

#include <desktop_multi_window/desktop_multi_window_plugin.h>

#include "flutter/generated_plugin_registrant.h"

struct _MyApplication {
  GtkApplication parent_instance;
  char** dart_entrypoint_arguments;
  FlMethodChannel* host_channel;
};

G_DEFINE_TYPE(MyApplication, my_application, GTK_TYPE_APPLICATION)

void host_channel_call_handler(FlMethodChannel* channel, FlMethodCall* method_call, gpointer user_data);

GtkWidget *find_gl_area(GtkWidget *widget);
void try_set_transparent(GtkWindow* window, GdkScreen* screen, FlView* view);

extern bool gIsConnectionManager;

// --- Side mouse button support (back/forward) ---
// Flutter's Linux embedder doesn't deliver X11 button 8/9 events to Dart.
// We intercept them via GDK and forward through a dedicated platform channel.

static const char* kSideButtonChannelName = "org.rustdesk.rustdesk/side_buttons";

static gboolean on_side_button_event(GtkWidget* widget, GdkEventButton* event, gpointer user_data) {
  if (event->button != 8 && event->button != 9) {
    return FALSE;
  }
  // Ignore GDK_2BUTTON_PRESS / GDK_3BUTTON_PRESS (double/triple-click synthetic
  // events) - only handle real press and release.
  if (event->type != GDK_BUTTON_PRESS && event->type != GDK_BUTTON_RELEASE) {
    return FALSE;
  }

  FlMethodChannel* channel = FL_METHOD_CHANNEL(user_data);
  if (channel == NULL) return FALSE;

  g_autoptr(FlValue) args = fl_value_new_map();
  fl_value_set_string_take(args, "button",
    fl_value_new_string(event->button == 8 ? "back" : "forward"));
  fl_value_set_string_take(args, "type",
    fl_value_new_string(event->type == GDK_BUTTON_PRESS ? "down" : "up"));

  fl_method_channel_invoke_method(channel, "onSideMouseButton", args,
    NULL, NULL, NULL);

  return TRUE;
}

static FlMethodChannel* side_buttons_create_channel(FlEngine* engine) {
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  return fl_method_channel_new(
    fl_engine_get_binary_messenger(engine),
    kSideButtonChannelName,
    FL_METHOD_CODEC(codec));
}

static void side_buttons_channel_destroy(gpointer data) {
  g_object_unref(data);
}

static void side_buttons_init_for_window(GtkWindow* window, FlMethodChannel* channel) {
  // Guard against double-initialization (would leave dangling signal user_data).
  if (g_object_get_data(G_OBJECT(window), "side-buttons-channel") != NULL) return;

  gtk_widget_add_events(GTK_WIDGET(window),
    GDK_BUTTON_PRESS_MASK | GDK_BUTTON_RELEASE_MASK);
  // Store channel on the window so it stays alive and is freed with the window.
  g_object_set_data_full(G_OBJECT(window), "side-buttons-channel",
    g_object_ref(channel), side_buttons_channel_destroy);
  g_signal_connect(window, "button-press-event",
    G_CALLBACK(on_side_button_event), channel);
  g_signal_connect(window, "button-release-event",
    G_CALLBACK(on_side_button_event), channel);
}

static void on_subwindow_created(FlPluginRegistry* registry) {
#if defined(GDK_WINDOWING_WAYLAND) && defined(HAS_KEYBOARD_SHORTCUTS_INHIBIT)
  wayland_shortcuts_inhibit_init_for_subwindow(registry);
#endif
  // Set up side button forwarding for sub-windows.
  if (registry == NULL || !FL_IS_VIEW(registry)) return;
  FlView* view = FL_VIEW(registry);
  GtkWidget* toplevel = gtk_widget_get_toplevel(GTK_WIDGET(view));
  if (toplevel != NULL && GTK_IS_WINDOW(toplevel)) {
    FlMethodChannel* channel = side_buttons_create_channel(fl_view_get_engine(view));
    if (channel == NULL) return;
    side_buttons_init_for_window(GTK_WINDOW(toplevel), channel);
    g_object_unref(channel);  // window now owns a ref via g_object_set_data_full
  }
}

GtkWidget *find_gl_area(GtkWidget *widget);

// Implements GApplication::activate.
static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);

  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));
  gtk_window_set_decorated(window, FALSE);
  // try setting icon for rustdesk, which uses the system cache
  GtkIconTheme* theme = gtk_icon_theme_get_default();
  gint icons[4] = {256, 128, 64, 32};
  for (int i = 0; i < 4; i++) {
    GdkPixbuf* icon = gtk_icon_theme_load_icon(theme, "rustdesk", icons[i], GTK_ICON_LOOKUP_NO_SVG, NULL);
    if (icon != nullptr) {
      gtk_window_set_icon(window, icon);
    }
  }
  // Use a header bar when running in GNOME as this is the common style used
  // by applications and is the setup most users will be using (e.g. Ubuntu
  // desktop).
  // If running on X and not using GNOME then just use a traditional title bar
  // in case the window manager does more exotic layout, e.g. tiling.
  // If running on Wayland assume the header bar will work (may need changing
  // if future cases occur).
  gboolean use_header_bar = TRUE;
  GdkScreen* screen = NULL;
#ifdef GDK_WINDOWING_X11
  screen = gtk_window_get_screen(window);
  if (screen != NULL && GDK_IS_X11_SCREEN(screen)) {
    const gchar* wm_name = gdk_x11_screen_get_window_manager_name(screen);
    if (g_strcmp0(wm_name, "GNOME Shell") != 0) {
      use_header_bar = FALSE;
    }
  }
#endif
  if (use_header_bar) {
    GtkHeaderBar* header_bar = GTK_HEADER_BAR(gtk_header_bar_new());
    gtk_widget_show(GTK_WIDGET(header_bar));
    gtk_header_bar_set_title(header_bar, "rustdesk");
    gtk_header_bar_set_show_close_button(header_bar, TRUE);
    gtk_window_set_titlebar(window, GTK_WIDGET(header_bar));
  } else {
    gtk_window_set_title(window, "rustdesk");
  }

  // auto bdw = bitsdojo_window_from(window); // <--- add this line
  // bdw->setCustomFrame(true);               // <-- add this line
  int width = 800, height = 600;
  if (gIsConnectionManager) {
    width = 300;
    height = 490;
  }
  gtk_window_set_default_size(window, width, height);   // <-- comment this line
  // gtk_widget_show(GTK_WIDGET(window));
  gtk_widget_set_opacity(GTK_WIDGET(window), 0);

  g_autoptr(FlDartProject) project = fl_dart_project_new();
  fl_dart_project_set_dart_entrypoint_arguments(project, self->dart_entrypoint_arguments);

  FlView* view = fl_view_new(project);
  gtk_container_add(GTK_CONTAINER(window), GTK_WIDGET(view));

  try_set_transparent(window, gtk_window_get_screen(window), view);
  gtk_widget_show(GTK_WIDGET(window));
  gtk_widget_show(GTK_WIDGET(view));

  // Register callback for sub-windows created by desktop_multi_window plugin.
  // Handles both Wayland shortcuts inhibition (guarded inside) and side button
  // forwarding. Safe to call on X11-only builds - the plugin just stores the
  // callback pointer regardless of windowing system.
  desktop_multi_window_plugin_set_window_created_callback(
      (WindowCreatedCallback)on_subwindow_created);

  fl_register_plugins(FL_PLUGIN_REGISTRY(view));

  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  self->host_channel = fl_method_channel_new(
    fl_engine_get_binary_messenger(fl_view_get_engine(view)),
    "org.rustdesk.rustdesk/host",
    FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(
    self->host_channel,
    host_channel_call_handler,
    self,
    nullptr);

  // Forward side mouse button events (back/forward) to Dart on the main window.
  FlMethodChannel* side_channel = side_buttons_create_channel(fl_view_get_engine(view));
  side_buttons_init_for_window(window, side_channel);
  g_object_unref(side_channel);

  gtk_widget_grab_focus(GTK_WIDGET(view));
}

// Implements GApplication::local_command_line.
static gboolean my_application_local_command_line(GApplication* application, gchar*** arguments, int* exit_status) {
  MyApplication* self = MY_APPLICATION(application);
  // Strip out the first argument as it is the binary name.
  self->dart_entrypoint_arguments = g_strdupv(*arguments + 1);

  g_autoptr(GError) error = nullptr;
  if (!g_application_register(application, nullptr, &error)) {
     g_warning("Failed to register: %s", error->message);
     *exit_status = 1;
     return TRUE;
  }

  g_application_activate(application);
  *exit_status = 0;

  return TRUE;
}

// Implements GObject::dispose.
static void my_application_dispose(GObject* object) {
  MyApplication* self = MY_APPLICATION(object);
  g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
  g_clear_object(&self->host_channel);
  G_OBJECT_CLASS(my_application_parent_class)->dispose(object);
}

static void my_application_class_init(MyApplicationClass* klass) {
  G_APPLICATION_CLASS(klass)->activate = my_application_activate;
  G_APPLICATION_CLASS(klass)->local_command_line = my_application_local_command_line;
  G_OBJECT_CLASS(klass)->dispose = my_application_dispose;
}

static void my_application_init(MyApplication* self) {}

MyApplication* my_application_new() {
  return MY_APPLICATION(g_object_new(my_application_get_type(),
                                     "application-id", APPLICATION_ID,
                                     "flags", G_APPLICATION_NON_UNIQUE,
                                     nullptr));
}

void host_channel_call_handler(FlMethodChannel* channel, FlMethodCall* method_call, gpointer user_data)
{
  if (strcmp(fl_method_call_get_name(method_call), "bumpMouse") == 0) {
    FlValue *args = fl_method_call_get_args(method_call);

    FlValue *dxValue = nullptr;
    FlValue *dyValue = nullptr;

    switch (fl_value_get_type(args))
    {
      case FL_VALUE_TYPE_MAP:
      {
        dxValue = fl_value_lookup_string(args, "dx");
        dyValue = fl_value_lookup_string(args, "dy");

        break;
      }
      case FL_VALUE_TYPE_LIST:
      {
        int listSize = fl_value_get_length(args);

        dxValue = (listSize >= 1) ? fl_value_get_list_value(args, 0) : nullptr;
        dyValue = (listSize >= 2) ? fl_value_get_list_value(args, 1) : nullptr;

        break;
      }

      default: break;
    }

    int dx = 0, dy = 0;

    if (dxValue && (fl_value_get_type(dxValue) == FL_VALUE_TYPE_INT)) {
      dx = fl_value_get_int(dxValue);
    }

    if (dyValue && (fl_value_get_type(dyValue) == FL_VALUE_TYPE_INT)) {
      dy = fl_value_get_int(dyValue);
    }

    bool result = bump_mouse(dx, dy);

    FlValue *result_value = fl_value_new_bool(result);

    GError *error = nullptr;

    if (!fl_method_call_respond_success(method_call, result_value, &error)) {
      g_warning("Failed to send Flutter Platform Channel response: %s", error->message);
      g_error_free(error);
    }

    fl_value_unref(result_value);
  }
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

// https://github.com/flutter/flutter/issues/152154
// Remove this workaround when flutter version is updated.
void try_set_transparent(GtkWindow* window, GdkScreen* screen, FlView* view)
{
  GtkWidget *gl_area = NULL;

  printf("Try setting transparent\n");

  gl_area = find_gl_area(GTK_WIDGET(view));
  if (gl_area != NULL) {
    gtk_gl_area_set_has_alpha(GTK_GL_AREA(gl_area), TRUE);
  }

  if (screen != NULL) {
    GdkVisual *visual = NULL;
    gtk_widget_set_app_paintable(GTK_WIDGET(window), TRUE);
    visual = gdk_screen_get_rgba_visual(screen);
    if (visual != NULL && gdk_screen_is_composited(screen)) {
      gtk_widget_set_visual(GTK_WIDGET(window), visual);
    }
  }
}
