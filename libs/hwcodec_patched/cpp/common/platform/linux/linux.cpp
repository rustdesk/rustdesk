#include "linux.h"
#include "../../log.h"
#include <cstring>
#include <dlfcn.h>
#include <dynlink_cuda.h>
#include <dynlink_loader.h>
#include <errno.h>
#include <exception> // Include the necessary header file
#include <signal.h>
#include <sys/prctl.h>

namespace
{
  void load_driver(CudaFunctions **pp_cuda_dl, NvencFunctions **pp_nvenc_dl,
                   CuvidFunctions **pp_cvdl)
  {
    if (cuda_load_functions(pp_cuda_dl, NULL) < 0)
    {
      LOG_TRACE(std::string("cuda_load_functions failed"));
      throw "cuda_load_functions failed";
    }
    if (nvenc_load_functions(pp_nvenc_dl, NULL) < 0)
    {
      LOG_TRACE(std::string("nvenc_load_functions failed"));
      throw "nvenc_load_functions failed";
    }
    if (cuvid_load_functions(pp_cvdl, NULL) < 0)
    {
      LOG_TRACE(std::string("cuvid_load_functions failed"));
      throw "cuvid_load_functions failed";
    }
  }

  void free_driver(CudaFunctions **pp_cuda_dl, NvencFunctions **pp_nvenc_dl,
                   CuvidFunctions **pp_cvdl)
  {
    if (*pp_cvdl)
    {
      cuvid_free_functions(pp_cvdl);
      *pp_cvdl = NULL;
    }
    if (*pp_nvenc_dl)
    {
      nvenc_free_functions(pp_nvenc_dl);
      *pp_nvenc_dl = NULL;
    }
    if (*pp_cuda_dl)
    {
      cuda_free_functions(pp_cuda_dl);
      *pp_cuda_dl = NULL;
    }
  }
} // namespace

int linux_support_nv()
{
  try
  {
    CudaFunctions *cuda_dl = NULL;
    NvencFunctions *nvenc_dl = NULL;
    CuvidFunctions *cvdl = NULL;
    load_driver(&cuda_dl, &nvenc_dl, &cvdl);
    free_driver(&cuda_dl, &nvenc_dl, &cvdl);
    return 0;
  }
  catch (...)
  {
    LOG_TRACE(std::string("nvidia driver not support"));
  }
  return -1;
}

int linux_support_amd()
{
#if defined(__x86_64__) || defined(__aarch64__)
#define AMF_DLL_NAME L"libamfrt64.so.1"
#define AMF_DLL_NAMEA "libamfrt64.so.1"
#else
#define AMF_DLL_NAME L"libamfrt32.so.1"
#define AMF_DLL_NAMEA "libamfrt32.so.1"
#endif
  void *handle = dlopen(AMF_DLL_NAMEA, RTLD_LAZY);
  if (!handle)
  {
    return -1;
  }
  dlclose(handle);
  return 0;
}

int linux_support_intel()
{
  const char *libs[] =
      {"libvpl.so", "libmfx.so", "libmfx-gen.so.1.2", "libmfxhw64.so.1"};
  for (size_t i = 0; i < sizeof(libs) / sizeof(libs[0]); i++)
  {
    void *handle = dlopen(libs[i], RTLD_LAZY);
    if (handle)
    {
      dlclose(handle);
      return 0;
    }
  }
  return -1;
}

int setup_parent_death_signal() {
  // Set up parent death signal to ensure this process dies if parent dies
  // This prevents orphaned processes especially when running with different
  // user permissions
  int ret = prctl(PR_SET_PDEATHSIG, SIGKILL);
  if (ret != 0) {
    LOG_ERROR(std::string("Failed to set parent death signal:") + std::to_string(errno));
    return -1;
  } else {
    return 0;
  }
}