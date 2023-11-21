vcpkg_configure_cmake(
    SOURCE_PATH "${CMAKE_CURRENT_LIST_DIR}"
    OPTIONS
    -DCURRENT_INSTALLED_DIR=${CURRENT_INSTALLED_DIR}
    PREFER_NINJA
)

vcpkg_cmake_install()
