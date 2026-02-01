#include "common.h"
#include <iostream>
#include <public/common/TraceAdapter.h>
#include <stdio.h>

#ifndef AMF_FACILITY
#define AMF_FACILITY L"AMFCommon"
#endif

static bool convert_api(amf::AMF_MEMORY_TYPE &rhs) {
  // Always use DX11 since it's the only supported API
  rhs = amf::AMF_MEMORY_DX11;
  return true;
}

static bool convert_surface_format(SurfaceFormat lhs,
                                   amf::AMF_SURFACE_FORMAT &rhs) {
  switch (lhs) {
  case SURFACE_FORMAT_NV12:
    rhs = amf::AMF_SURFACE_NV12;
    break;
  case SURFACE_FORMAT_RGBA:
    rhs = amf::AMF_SURFACE_RGBA;
    break;
  case SURFACE_FORMAT_BGRA:
    rhs = amf::AMF_SURFACE_BGRA;
    break;
  default:
    std::cerr << "unsupported surface format: " << static_cast<int>(lhs)
              << "\n";
    return false;
  }
  return true;
}
