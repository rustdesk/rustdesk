cmake_minimum_required(VERSION 3.20)
project(oboe_wrapper CXX)

include(GNUInstallDirs)

add_library(oboe_wrapper STATIC
    oboe.cc
)

target_include_directories(oboe_wrapper PRIVATE "${CURRENT_INSTALLED_DIR}/include")

install(TARGETS oboe_wrapper
        ARCHIVE DESTINATION "${CMAKE_INSTALL_LIBDIR}"
        LIBRARY DESTINATION "${CMAKE_INSTALL_LIBDIR}"
        RUNTIME DESTINATION "${CMAKE_INSTALL_BINDIR}")
