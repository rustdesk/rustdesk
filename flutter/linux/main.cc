#include <dlfcn.h>
#include "my_application.h"

#define RUSTDESK_LIB_PATH "librustdesk.so"
typedef bool (*RustDeskCoreMain)();
bool gIsConnectionManager = false;

void print_help_install_pkg(const char* so);

bool flutter_rustdesk_core_main() {
   void* librustdesk = dlopen(RUSTDESK_LIB_PATH, RTLD_LAZY);
   if (!librustdesk) {
      fprintf(stderr,"Failed to load \"librustdesk.so\"\n");
      char* error;
      if ((error = dlerror()) != nullptr) {
        fprintf(stderr, "%s\n", error);
        char* libmissed = strstr(error, ": cannot open shared object file: No such file or directory");
        if (libmissed != nullptr) {
          *libmissed = '\0';
          char* so = strdup(error);
          print_help_install_pkg(so);
          free(so);
        }
      }
     return false;
   }
   auto core_main = (RustDeskCoreMain) dlsym(librustdesk,"rustdesk_core_main");
   char* error;
   if ((error = dlerror()) != nullptr) {
       fprintf(stderr, "Program entry \"rustdesk_core_main\" is not found: %s\n", error);
       return false;
   }
   return core_main();
}

int main(int argc, char** argv) {
  if (!flutter_rustdesk_core_main()) {
      return 0;
  }
  for (int i = 0; i < argc; i++) {
    if (strcmp(argv[i], "--cm") == 0) {
      gIsConnectionManager = true;
    }
  }
  g_autoptr(MyApplication) app = my_application_new();
  return g_application_run(G_APPLICATION(app), argc, argv);
}

typedef struct {
  const char* mgr;
  const char* search;
} PkgMgrSearch;

const PkgMgrSearch g_mgrs[] = {
  {
    "apt",
    "apt-file search",
  },
  {
    "dnf",
    "dnf provides",
  },
  {
    "yum",
    "yum provides",
  },
  {
    "zypper",
    "zypper wp",
  },
  {
    "pacman",
    "pacman -Qo",
  },
  {
    NULL,
    NULL,
  },
};

int is_command_exists(const char* command) {
    char* path = getenv("PATH");
    char* path_copy = strdup(path);
    char* dir = strtok(path_copy, ":");

    while (dir != NULL) {
        char command_path[256];
        snprintf(command_path, sizeof(command_path), "%s/%s", dir, command);
        if (access(command_path, X_OK) == 0) {
            free(path_copy);
            return 1;
        }
        dir = strtok(NULL, ":");
    }

    free(path_copy);
    return 0;
}

// We do not automatically search pkg 
// as the search process can be time consuming and update may be required.
void print_help_install_pkg(const char* so)
{
  if (strcmp(so, "libnsl.so.1") == 0) {
    const char* mgr[] = {"yum", "dnf", NULL};
    const char** m = mgr;
    while (*m != NULL) {
      if (is_command_exists(*m)) {
        fprintf(stderr, "Please run \"%s install libnsl\" to install the required package.\n", *m);
        return;
      }
      m++;
    }
  }

  const PkgMgrSearch *mgr_search = g_mgrs;
  while (mgr_search->mgr != NULL) {
      if (is_command_exists(mgr_search->mgr) == 1) {
        fprintf(stderr, "Please run \"%s %s\" to search and install the pkg.\n", mgr_search->search, so);
        break;
      }
      mgr_search++;
  }
}
