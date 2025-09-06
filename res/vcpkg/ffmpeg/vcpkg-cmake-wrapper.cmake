set(FFMPEG_PREV_MODULE_PATH ${CMAKE_MODULE_PATH})
list(APPEND CMAKE_MODULE_PATH ${CMAKE_CURRENT_LIST_DIR})

include(SelectLibraryConfigurations)

cmake_policy(SET CMP0012 NEW)

set(vcpkg_no_avcodec_target ON)
set(vcpkg_no_avformat_target ON)
set(vcpkg_no_avutil_target ON)
if(TARGET FFmpeg::avcodec)
  set(vcpkg_no_avcodec_target OFF)
endif()
if(TARGET FFmpeg::avformat)
  set(vcpkg_no_avformat_target OFF)
endif()
if(TARGET FFmpeg::avutil)
  set(vcpkg_no_avutil_target OFF)
endif()

_find_package(${ARGS})

if(WIN32)
  set(PKG_CONFIG_EXECUTABLE "${CMAKE_CURRENT_LIST_DIR}/../../../@_HOST_TRIPLET@/tools/pkgconf/pkgconf.exe" CACHE STRING "" FORCE)
endif()

set(PKG_CONFIG_USE_CMAKE_PREFIX_PATH ON) # Required for CMAKE_MINIMUM_REQUIRED_VERSION VERSION_LESS 3.1 which otherwise ignores CMAKE_PREFIX_PATH

if(@WITH_MFX@)
  find_package(PkgConfig )
  pkg_check_modules(libmfx  IMPORTED_TARGET libmfx)
  list(APPEND FFMPEG_LIBRARIES PkgConfig::libmfx)
  if(vcpkg_no_avcodec_target AND TARGET FFmpeg::avcodec)
    target_link_libraries(FFmpeg::avcodec INTERFACE PkgConfig::libmfx)
  endif()
  if(vcpkg_no_avutil_target AND TARGET FFmpeg::avutil)
    target_link_libraries(FFmpeg::avutil INTERFACE PkgConfig::libmfx)
  endif()
endif()

set(FFMPEG_LIBRARY ${FFMPEG_LIBRARIES})

set(CMAKE_MODULE_PATH ${FFMPEG_PREV_MODULE_PATH})

unset(vcpkg_no_avformat_target)
unset(vcpkg_no_avcodec_target)
unset(vcpkg_no_avutil_target)
