vcpkg_check_linkage(ONLY_STATIC_LIBRARY)

vcpkg_from_github(
    OUT_SOURCE_PATH SOURCE_PATH
    REPO webmproject/libvpx
    REF "v${VERSION}"
    SHA512 3e3bfad3d035c0bc3db7cb5a194d56d3c90f5963fb1ad527ae5252054e7c48ce2973de1346c97d94b59f7a95d4801bec44214cce10faf123f92b36fca79a8d1e
    HEAD_REF master
    PATCHES
        0002-Fix-nasm-debug-format-flag.patch
        0003-add-uwp-v142-and-v143-support.patch
        0004-remove-library-suffixes.patch
)

if(CMAKE_HOST_WIN32)
    vcpkg_acquire_msys(MSYS_ROOT PACKAGES make perl)
    set(ENV{PATH} "${MSYS_ROOT}/usr/bin;$ENV{PATH}")
else()
    vcpkg_find_acquire_program(PERL)
    get_filename_component(PERL_EXE_PATH ${PERL} DIRECTORY)
    set(ENV{PATH} "${MSYS_ROOT}/usr/bin:$ENV{PATH}:${PERL_EXE_PATH}")
endif()

find_program(BASH NAME bash HINTS ${MSYS_ROOT}/usr/bin REQUIRED NO_CACHE)

vcpkg_find_acquire_program(NASM)
get_filename_component(NASM_EXE_PATH ${NASM} DIRECTORY)
vcpkg_add_to_path(${NASM_EXE_PATH})

if(VCPKG_TARGET_IS_WINDOWS AND NOT VCPKG_TARGET_IS_MINGW)

    file(REMOVE_RECURSE "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-tmp")

    if(VCPKG_CRT_LINKAGE STREQUAL static)
        set(LIBVPX_CRT_LINKAGE --enable-static-msvcrt)
        set(LIBVPX_CRT_SUFFIX mt)
    else()
        set(LIBVPX_CRT_SUFFIX md)
    endif()

    if(VCPKG_CMAKE_SYSTEM_NAME STREQUAL WindowsStore AND (VCPKG_PLATFORM_TOOLSET STREQUAL v142 OR VCPKG_PLATFORM_TOOLSET STREQUAL v143))
        set(LIBVPX_TARGET_OS "uwp")
    elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL x86 OR VCPKG_TARGET_ARCHITECTURE STREQUAL arm)
        set(LIBVPX_TARGET_OS "win32")
    elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL x64 OR VCPKG_TARGET_ARCHITECTURE STREQUAL arm64)
        set(LIBVPX_TARGET_OS "win64")
    endif()

    if(VCPKG_TARGET_ARCHITECTURE STREQUAL x86)
        set(LIBVPX_TARGET_ARCH "x86-${LIBVPX_TARGET_OS}")
        set(LIBVPX_ARCH_DIR "Win32")
    elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL x64)
        set(LIBVPX_TARGET_ARCH "x86_64-${LIBVPX_TARGET_OS}")
        set(LIBVPX_ARCH_DIR "x64")
    elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL arm64)
        set(LIBVPX_TARGET_ARCH "arm64-${LIBVPX_TARGET_OS}")
        set(LIBVPX_ARCH_DIR "ARM64")
    elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL arm)
        set(LIBVPX_TARGET_ARCH "armv7-${LIBVPX_TARGET_OS}")
        set(LIBVPX_ARCH_DIR "ARM")
    endif()

    if(VCPKG_PLATFORM_TOOLSET STREQUAL v143)
        set(LIBVPX_TARGET_VS "vs17")
    elseif(VCPKG_PLATFORM_TOOLSET STREQUAL v142)
        set(LIBVPX_TARGET_VS "vs16")
    else()
        set(LIBVPX_TARGET_VS "vs15")
    endif()

    set(OPTIONS "--disable-examples --disable-tools --disable-docs --enable-pic")

    if("realtime" IN_LIST FEATURES)
        set(OPTIONS "${OPTIONS} --enable-realtime-only")
    endif()

    if("highbitdepth" IN_LIST FEATURES)
        set(OPTIONS "${OPTIONS} --enable-vp9-highbitdepth")
    endif()

    message(STATUS "Generating makefile")
    file(MAKE_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-tmp")
    vcpkg_execute_required_process(
        COMMAND
            ${BASH} --noprofile --norc
            "${SOURCE_PATH}/configure"
            --target=${LIBVPX_TARGET_ARCH}-${LIBVPX_TARGET_VS}
            ${LIBVPX_CRT_LINKAGE}
            ${OPTIONS}
            --as=nasm
        WORKING_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-tmp"
        LOGNAME configure-${TARGET_TRIPLET})

    message(STATUS "Generating MSBuild projects")
    vcpkg_execute_required_process(
        COMMAND
            ${BASH} --noprofile --norc -c "make dist"
        WORKING_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-tmp"
        LOGNAME generate-${TARGET_TRIPLET})

    vcpkg_msbuild_install(
        SOURCE_PATH "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-tmp"
        PROJECT_SUBPATH vpx.vcxproj
    )

    if (VCPKG_TARGET_ARCHITECTURE STREQUAL arm64)
        set(LIBVPX_INCLUDE_DIR "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-rel/vpx-vp8-vp9-nopost-nodocs-${LIBVPX_TARGET_ARCH}${LIBVPX_CRT_SUFFIX}-${LIBVPX_TARGET_VS}-v${VERSION}/include/vpx")
    elseif (VCPKG_TARGET_ARCHITECTURE STREQUAL arm)
        set(LIBVPX_INCLUDE_DIR "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-rel/vpx-vp8-vp9-nopost-nomt-nodocs-${LIBVPX_TARGET_ARCH}${LIBVPX_CRT_SUFFIX}-${LIBVPX_TARGET_VS}-v${VERSION}/include/vpx")
    else()
        set(LIBVPX_INCLUDE_DIR "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-rel/vpx-vp8-vp9-nodocs-${LIBVPX_TARGET_ARCH}${LIBVPX_CRT_SUFFIX}-${LIBVPX_TARGET_VS}-v${VERSION}/include/vpx")
    endif()
    file(
        INSTALL
            "${LIBVPX_INCLUDE_DIR}"
        DESTINATION
            "${CURRENT_PACKAGES_DIR}/include"
        RENAME
            "vpx")
    if (NOT DEFINED VCPKG_BUILD_TYPE OR VCPKG_BUILD_TYPE STREQUAL "release")
        set(LIBVPX_PREFIX "${CURRENT_INSTALLED_DIR}")
        configure_file("${CMAKE_CURRENT_LIST_DIR}/vpx.pc.in" "${CURRENT_PACKAGES_DIR}/lib/pkgconfig/vpx.pc" @ONLY)
    endif()

    if (NOT DEFINED VCPKG_BUILD_TYPE OR VCPKG_BUILD_TYPE STREQUAL "debug")
        set(LIBVPX_PREFIX "${CURRENT_INSTALLED_DIR}/debug")
        configure_file("${CMAKE_CURRENT_LIST_DIR}/vpx.pc.in" "${CURRENT_PACKAGES_DIR}/debug/lib/pkgconfig/vpx.pc" @ONLY)
    endif()

else()

    set(OPTIONS "--disable-examples --disable-tools --disable-docs --disable-unit-tests --enable-pic")

    set(OPTIONS_DEBUG "--enable-debug-libs --enable-debug --prefix=${CURRENT_PACKAGES_DIR}/debug")
    set(OPTIONS_RELEASE "--prefix=${CURRENT_PACKAGES_DIR}")
    set(AS_NASM "--as=nasm")

    if(VCPKG_LIBRARY_LINKAGE STREQUAL "dynamic")
        set(OPTIONS "${OPTIONS} --disable-static --enable-shared")
    else()
        set(OPTIONS "${OPTIONS} --enable-static --disable-shared")
    endif()

    if("realtime" IN_LIST FEATURES)
        set(OPTIONS "${OPTIONS} --enable-realtime-only")
    endif()

    if("highbitdepth" IN_LIST FEATURES)
        set(OPTIONS "${OPTIONS} --enable-vp9-highbitdepth")
    endif()

    if(VCPKG_TARGET_ARCHITECTURE STREQUAL x86)
        set(LIBVPX_TARGET_ARCH "x86")
    elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL x64)
        set(LIBVPX_TARGET_ARCH "x86_64")
    elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL arm)
        set(LIBVPX_TARGET_ARCH "armv7")
    elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL arm64)
        set(LIBVPX_TARGET_ARCH "arm64")
    else()
        message(FATAL_ERROR "libvpx does not support architecture ${VCPKG_TARGET_ARCHITECTURE}")
    endif()

    vcpkg_cmake_get_vars(cmake_vars_file)
    include("${cmake_vars_file}")

    # Set environment variables for configure
    if(VCPKG_DETECTED_CMAKE_C_COMPILER MATCHES "([^\/]*-)gcc$")
        message(STATUS "Cross-building for ${TARGET_TRIPLET} with ${CMAKE_MATCH_1}")
        set(ENV{CROSS} ${CMAKE_MATCH_1})
        unset(AS_NASM)
    else()
        set(ENV{CC} ${VCPKG_DETECTED_CMAKE_C_COMPILER})
        set(ENV{CXX} ${VCPKG_DETECTED_CMAKE_CXX_COMPILER})
        set(ENV{AR} ${VCPKG_DETECTED_CMAKE_AR})
        set(ENV{LD} ${VCPKG_DETECTED_CMAKE_LINKER})
        set(ENV{RANLIB} ${VCPKG_DETECTED_CMAKE_RANLIB})
        set(ENV{STRIP} ${VCPKG_DETECTED_CMAKE_STRIP})
    endif()

    if(VCPKG_TARGET_IS_MINGW)
        if(LIBVPX_TARGET_ARCH STREQUAL "x86")
            set(LIBVPX_TARGET "x86-win32-gcc")
        else()
            set(LIBVPX_TARGET "x86_64-win64-gcc")
        endif()
    elseif(VCPKG_TARGET_IS_LINUX)
        set(LIBVPX_TARGET "${LIBVPX_TARGET_ARCH}-linux-gcc")
    elseif(VCPKG_TARGET_IS_ANDROID)
        set(LIBVPX_TARGET "generic-gnu")
        # Settings
        if(VCPKG_TARGET_ARCHITECTURE STREQUAL x86)
            set(OPTIONS "${OPTIONS} --disable-sse4_1 --disable-avx --disable-avx2 --disable-avx512")
        elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL x64)
            set(OPTIONS "${OPTIONS} --disable-avx --disable-avx2 --disable-avx512")
        elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL arm)
            set(OPTIONS "${OPTIONS} --enable-thumb --disable-neon")
        elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL arm64)
            set(OPTIONS "${OPTIONS} --enable-thumb")
        endif()
        # Set environment variables for configure
        set(ENV{AS} ${VCPKG_DETECTED_CMAKE_C_COMPILER})
        set(ENV{LDFLAGS} "${LDFLAGS} --target=${VCPKG_DETECTED_CMAKE_C_COMPILER_TARGET}")
        # Set clang target
        set(OPTIONS "${OPTIONS} --extra-cflags=--target=${VCPKG_DETECTED_CMAKE_C_COMPILER_TARGET} --extra-cxxflags=--target=${VCPKG_DETECTED_CMAKE_CXX_COMPILER_TARGET}")
        # Unset nasm and let AS do its job
        unset(AS_NASM)
    elseif(VCPKG_TARGET_IS_OSX)
        if(VCPKG_TARGET_ARCHITECTURE STREQUAL "arm64")
            set(LIBVPX_TARGET "arm64-darwin20-gcc")
            if(DEFINED VCPKG_OSX_DEPLOYMENT_TARGET)
                set(MAC_OSX_MIN_VERSION_CFLAGS --extra-cflags=-mmacosx-version-min=${VCPKG_OSX_DEPLOYMENT_TARGET} --extra-cxxflags=-mmacosx-version-min=${VCPKG_OSX_DEPLOYMENT_TARGET})
            endif()
        else()
            set(LIBVPX_TARGET "${LIBVPX_TARGET_ARCH}-darwin17-gcc") # enable latest CPU instructions for best performance and less CPU usage on MacOS
        endif()
    elseif(VCPKG_TARGET_IS_IOS)
        if(VCPKG_TARGET_ARCHITECTURE STREQUAL arm)
            set(LIBVPX_TARGET "armv7-darwin-gcc")
        elseif(VCPKG_TARGET_ARCHITECTURE STREQUAL arm64)
            set(LIBVPX_TARGET "arm64-darwin-gcc")
        else()
            message(FATAL_ERROR "libvpx does not support architecture ${VCPKG_TARGET_ARCHITECTURE} on iOS")
        endif()
    else()
        set(LIBVPX_TARGET "generic-gnu") # use default target
    endif()

    message(STATUS "Build info. Target: ${LIBVPX_TARGET}; Options: ${OPTIONS}")

    if(NOT DEFINED VCPKG_BUILD_TYPE OR VCPKG_BUILD_TYPE STREQUAL "release")
        message(STATUS "Configuring libvpx for Release")
        file(MAKE_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-rel")
        vcpkg_execute_required_process(
        COMMAND
            ${BASH} --noprofile --norc
            "${SOURCE_PATH}/configure"
            --target=${LIBVPX_TARGET}
            ${OPTIONS}
            ${OPTIONS_RELEASE}
            ${MAC_OSX_MIN_VERSION_CFLAGS}
            ${AS_NASM}
        WORKING_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-rel"
        LOGNAME configure-${TARGET_TRIPLET}-rel)

        message(STATUS "Building libvpx for Release")
        vcpkg_execute_required_process(
            COMMAND
                ${BASH} --noprofile --norc -c "make -j${VCPKG_CONCURRENCY}"
            WORKING_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-rel"
            LOGNAME build-${TARGET_TRIPLET}-rel
        )

        message(STATUS "Installing libvpx for Release")
        vcpkg_execute_required_process(
            COMMAND
                ${BASH} --noprofile --norc -c "make install"
            WORKING_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-rel"
            LOGNAME install-${TARGET_TRIPLET}-rel
        )
    endif()

    # --- --- ---

    if(NOT DEFINED VCPKG_BUILD_TYPE OR VCPKG_BUILD_TYPE STREQUAL "debug")
        message(STATUS "Configuring libvpx for Debug")
        file(MAKE_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-dbg")
        vcpkg_execute_required_process(
        COMMAND
            ${BASH} --noprofile --norc
            "${SOURCE_PATH}/configure"
            --target=${LIBVPX_TARGET}
            ${OPTIONS}
            ${OPTIONS_DEBUG}
            ${MAC_OSX_MIN_VERSION_CFLAGS}
            ${AS_NASM}
        WORKING_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-dbg"
        LOGNAME configure-${TARGET_TRIPLET}-dbg)

        message(STATUS "Building libvpx for Debug")
        vcpkg_execute_required_process(
            COMMAND
                ${BASH} --noprofile --norc -c "make -j${VCPKG_CONCURRENCY}"
            WORKING_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-dbg"
            LOGNAME build-${TARGET_TRIPLET}-dbg
        )

        message(STATUS "Installing libvpx for Debug")
        vcpkg_execute_required_process(
            COMMAND
                ${BASH} --noprofile --norc -c "make install"
            WORKING_DIRECTORY "${CURRENT_BUILDTREES_DIR}/${TARGET_TRIPLET}-dbg"
            LOGNAME install-${TARGET_TRIPLET}-dbg
        )

        file(REMOVE_RECURSE "${CURRENT_PACKAGES_DIR}/debug/include")
        file(REMOVE "${CURRENT_PACKAGES_DIR}/debug/lib/libvpx_g.a")
    endif()
endif()

vcpkg_fixup_pkgconfig()

if(NOT DEFINED VCPKG_BUILD_TYPE OR VCPKG_BUILD_TYPE STREQUAL "debug")
    set(LIBVPX_CONFIG_DEBUG ON)
else()
    set(LIBVPX_CONFIG_DEBUG OFF)
endif()

configure_file("${CMAKE_CURRENT_LIST_DIR}/unofficial-libvpx-config.cmake.in" "${CURRENT_PACKAGES_DIR}/share/unofficial-libvpx/unofficial-libvpx-config.cmake" @ONLY)

vcpkg_install_copyright(FILE_LIST "${SOURCE_PATH}/LICENSE")
