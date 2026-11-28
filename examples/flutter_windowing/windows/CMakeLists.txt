cmake_minimum_required(VERSION 3.14)
set(PROJECT_NAME "desktop_multi_window")
project(${PROJECT_NAME} LANGUAGES CXX)

# This value is used when generating builds using this plugin, so it must
# not be changed
set(PLUGIN_NAME "desktop_multi_window_plugin")

# Standalone official-backend harness: no RustDesk-specific legacy plugins.
# OFF also includes the RustDesk-specific legacy plugins.
option(DESKTOP_MULTI_WINDOW_OFFICIAL_ONLY "Build experimental official-only harness" OFF)
if(DESKTOP_MULTI_WINDOW_OFFICIAL_ONLY)
  add_library(${PLUGIN_NAME} SHARED "official_only_plugin.cpp" "official_window_controls.cpp")
  apply_standard_settings(${PLUGIN_NAME})
  target_compile_definitions(${PLUGIN_NAME} PRIVATE FLUTTER_PLUGIN_IMPL)
  target_include_directories(${PLUGIN_NAME} INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/include")
  target_link_libraries(${PLUGIN_NAME} PRIVATE flutter flutter_wrapper_plugin comctl32)
  set(desktop_multi_window_bundled_libraries "" PARENT_SCOPE)
  return()
endif()

add_library(${PLUGIN_NAME} SHARED
  "desktop_multi_window_plugin.cpp"
  "official_window_controls.cpp"
  "multi_window_manager.cc"
  "flutter_window.cc"
  "window_channel.cc"
  "icon.rc"
  "base_flutter_window.cc")
apply_standard_settings(${PLUGIN_NAME})
set_target_properties(${PLUGIN_NAME} PROPERTIES
  CXX_VISIBILITY_PRESET hidden)
target_compile_definitions(${PLUGIN_NAME} PRIVATE FLUTTER_PLUGIN_IMPL)
target_include_directories(${PLUGIN_NAME} INTERFACE
  "${CMAKE_CURRENT_SOURCE_DIR}/include")
target_link_libraries(${PLUGIN_NAME} PRIVATE flutter flutter_wrapper_plugin flutter_wrapper_app url_launcher_windows_plugin window_size_plugin texture_rgba_renderer_plugin comctl32)

# List of absolute paths to libraries that should be bundled with the plugin
set(desktop_multi_window_bundled_libraries
  ""
  PARENT_SCOPE
)
