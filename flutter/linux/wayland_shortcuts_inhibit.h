// Wayland keyboard shortcuts inhibit support
// This module provides functionality to inhibit system keyboard shortcuts
// on Wayland compositors, allowing remote desktop windows to capture all
// key events including Super, Alt+Tab, etc.

#ifndef WAYLAND_SHORTCUTS_INHIBIT_H_
#define WAYLAND_SHORTCUTS_INHIBIT_H_

#include <gtk/gtk.h>

#if defined(GDK_WINDOWING_WAYLAND) && defined(HAS_KEYBOARD_SHORTCUTS_INHIBIT)

// Initialize shortcuts inhibit for a sub-window created by desktop_multi_window plugin.
// This sets up focus-based inhibitor management: inhibitor is created when
// the window gains focus and destroyed when it loses focus.
//
// @param view The FlView of the sub-window
void wayland_shortcuts_inhibit_init_for_subwindow(void* view);

#endif  // defined(GDK_WINDOWING_WAYLAND) && defined(HAS_KEYBOARD_SHORTCUTS_INHIBIT)

#endif  // WAYLAND_SHORTCUTS_INHIBIT_H_
