#include "bump_mouse.h"

#include <gtk/gtk.h>

#include <gdk/gdkx.h>

#include <iostream>

bool bump_mouse_x11(int dx, int dy)
{
  GdkDevice *mouse_device;

#if GTK_CHECK_VERSION(3, 20, 0)
  auto seat = gdk_display_get_default_seat(gdk_display_get_default());

  mouse_device = gdk_seat_get_pointer(seat);
#else
  auto devman = gdk_display_get_device_manager(gdk_display_get_default());

  mouse_device = gdk_device_manager_get_client_pointer(devman);
#endif

  GdkScreen *screen;
  gint x, y;

  gdk_device_get_position(mouse_device, &screen, &x, &y);
  gdk_device_warp(mouse_device, screen, x + dx, y + dy);

  return true;
}
