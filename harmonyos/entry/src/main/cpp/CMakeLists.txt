# the minimum version of CMake.
cmake_minimum_required(VERSION 3.5.0)
project(rustdesk_entry)

set(NATIVERENDER_ROOT_PATH ${CMAKE_CURRENT_SOURCE_DIR})

if(DEFINED PACKAGE_FIND_FILE)
    include(${PACKAGE_FIND_FILE})
endif()

include_directories(${NATIVERENDER_ROOT_PATH})

add_library(entry SHARED napi_init.cpp)

target_compile_options(entry PRIVATE -std=c++17 -Wall -fvisibility=hidden)

# libnative_window.so / libnative_buffer.so: XComponent surface rendering.
# librustdesk.so is deliberately NOT linked here; napi_init.cpp resolves the
# Rust FFI symbols at runtime with dlopen/dlsym so the UI still boots (with
# graceful errors) before/without the Rust library.
target_link_libraries(entry PUBLIC
    libace_napi.z.so
    libnative_window.so
    libnative_buffer.so
    libhilog_ndk.z.so)
