vcpkg_from_gitlab(
    GITLAB_URL https://gitlab.com
    OUT_SOURCE_PATH SOURCE_PATH
    REPO AOMediaCodec/SVT-AV1
    REF v${VERSION}
    SHA512 37df96559af179d28acae10cdff98c94ee2c4ee086b2446ab362b57be9adf566d6f2adc28faa30648798d9bc7d18eaeb2d0665e7292982652496b605342e1c4b
    PATCHES
        # upstream forces llvm-ld/llvm-ar/llvm-ranlib for clang static builds on
        # non-Apple unix, which breaks toolchains without the llvm binutils
        no-force-llvm.diff
        # MSVC >= 1950 miscompiles the inlined yy_unpacklo_epi128
        no-inline-yy_unpacklo_epi128.diff
)

if(VCPKG_TARGET_ARCHITECTURE MATCHES "^(x86|x64)$")
    # NASM is required to build the x86/x64 assembly
    vcpkg_find_acquire_program(NASM)
    set(SIMD_OPTIONS -DCOMPILE_C_ONLY=OFF "-DCMAKE_ASM_NASM_COMPILER=${NASM}")
elseif(VCPKG_TARGET_ARCHITECTURE MATCHES "^(arm64|arm64ec)$" AND NOT VCPKG_TARGET_IS_WINDOWS)
    set(SIMD_OPTIONS -DCOMPILE_C_ONLY=OFF)
else()
    set(SIMD_OPTIONS -DCOMPILE_C_ONLY=ON)
endif()

vcpkg_cmake_configure(
    SOURCE_PATH "${SOURCE_PATH}"
    OPTIONS
        ${SIMD_OPTIONS}
        -DBUILD_APPS=OFF
        -DBUILD_TESTING=OFF
        -DREPRODUCIBLE_BUILDS=ON
        # SVT-AV1 defaults LTO to ON for gcc>=9/clang>=12, which puts LTO
        # bitcode into the static archive and breaks linking with a different
        # compiler version
        -DSVT_AV1_LTO=OFF
)

vcpkg_cmake_install()
vcpkg_cmake_config_fixup(PACKAGE_NAME SVT-AV1 CONFIG_PATH lib/cmake/SVT-AV1)
vcpkg_copy_pdbs()
vcpkg_fixup_pkgconfig()

file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/debug/include")

vcpkg_install_copyright(FILE_LIST "${SOURCE_PATH}/LICENSE.md" "${SOURCE_PATH}/PATENTS.md")
