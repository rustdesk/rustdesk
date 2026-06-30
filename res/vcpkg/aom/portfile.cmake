# NASM is required to build AOM
vcpkg_find_acquire_program(NASM)
get_filename_component(NASM_EXE_PATH ${NASM} DIRECTORY)
vcpkg_add_to_path(${NASM_EXE_PATH})

# Perl is required to build AOM
vcpkg_find_acquire_program(PERL)
get_filename_component(PERL_PATH ${PERL} DIRECTORY)
vcpkg_add_to_path(${PERL_PATH})

# Force AOM 3.9.1 for RustDesk Windows static build
vcpkg_from_git(
    OUT_SOURCE_PATH SOURCE_PATH
    URL "https://aomedia.googlesource.com/aom"
    REF 8ad484f8a18ed1853c094e7d3a4e023b2a92df28 # 3.9.1
    PATCHES
        aom-uninitialized-pointer.diff
        aom-avx2.diff
        aom-install.diff
)

set(aom_target_cpu "")
if(VCPKG_TARGET_IS_UWP OR (VCPKG_TARGET_IS_WINDOWS AND VCPKG_TARGET_ARCHITECTURE MATCHES "^arm"))
    set(aom_target_cpu "-DAOM_TARGET_CPU=generic")
endif()

if(VCPKG_TARGET_ARCHITECTURE STREQUAL "arm" AND VCPKG_TARGET_IS_LINUX)
    set(aom_target_cpu "-DENABLE_NEON=OFF")
endif()

vcpkg_cmake_configure(
    SOURCE_PATH ${SOURCE_PATH}
    OPTIONS
        ${aom_target_cpu}
        -DENABLE_DOCS=OFF
        -DENABLE_EXAMPLES=OFF
        -DENABLE_TESTDATA=OFF
        -DENABLE_TESTS=OFF
        -DENABLE_TOOLS=OFF
)

vcpkg_cmake_install()

vcpkg_copy_pdbs()

if(VCPKG_TARGET_IS_WINDOWS)
    file(REMOVE_RECURSE
        "${CURRENT_PACKAGES_DIR}/lib/pkgconfig"
        "${CURRENT_PACKAGES_DIR}/debug/lib/pkgconfig"
    )
else()
    vcpkg_fixup_pkgconfig()
endif()

vcpkg_cmake_config_fixup(CONFIG_PATH lib/cmake/${PORT})

file(REMOVE_RECURSE
    "${CURRENT_PACKAGES_DIR}/debug/include"
    "${CURRENT_PACKAGES_DIR}/debug/share"
)

file(INSTALL "${SOURCE_PATH}/LICENSE" DESTINATION "${CURRENT_PACKAGES_DIR}/share/${PORT}" RENAME copyright)