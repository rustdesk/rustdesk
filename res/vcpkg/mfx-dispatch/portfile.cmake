vcpkg_download_distfile(
    MISSING_CSTDINT_IMPORT_PATCH
    URLS https://github.com/lu-zero/mfx_dispatch/commit/d6241243f85a0d947bdfe813006686a930edef24.patch?full_index=1
    FILENAME fix-missing-cstdint-import-d6241243f85a0d947bdfe813006686a930edef24.patch
    SHA512 5d2ffc4ec2ba0e5859d01d2e072f75436ebc3e62e0f6580b5bb8b9f82fe588e7558a46a1fdfa0297a782c0eeb8f50322258d0dd9e41d927cc9be496727b61e44
)

vcpkg_from_github(
    OUT_SOURCE_PATH SOURCE_PATH
    REPO lu-zero/mfx_dispatch
    REF "${VERSION}"
    SHA512 12517338342d3e653043a57e290eb9cffd190aede0c3a3948956f1c7f12f0ea859361cf3e534ab066b96b1c211f68409c67ef21fd6d76b68cc31daef541941b0
    HEAD_REF master
    PATCHES
        fix-unresolved-symbol.patch
        fix-pkgconf.patch
        0003-upgrade-cmake-3.14.patch
        ${MISSING_CSTDINT_IMPORT_PATCH}
)

if(VCPKG_TARGET_IS_WINDOWS AND NOT VCPKG_TARGET_IS_MINGW)
    vcpkg_cmake_configure(
        SOURCE_PATH "${SOURCE_PATH}" 
    )
    vcpkg_cmake_install()
    vcpkg_copy_pdbs()
else()
    if(VCPKG_TARGET_IS_MINGW)
        vcpkg_check_linkage(ONLY_STATIC_LIBRARY)
    endif()
    vcpkg_configure_make(
        SOURCE_PATH "${SOURCE_PATH}"
        AUTOCONFIG
    )
    vcpkg_install_make()
endif()
vcpkg_fixup_pkgconfig()
  
file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/debug/include")
vcpkg_install_copyright(FILE_LIST "${SOURCE_PATH}/LICENSE")
