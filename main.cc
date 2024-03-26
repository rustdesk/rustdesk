#include <dlfcn.h>
#include <iostream>

int main()
{
    void *handle = dlopen("../Frameworks/liblibrustdesk.dylib", RTLD_LAZY);
    if (!handle)
    {
        std::cerr << "Cannot open library: " << dlerror() << '\n';
        return 1;
    }

    // use dlsym to get a symbol from the library
    typedef int (*some_func_t)();
    some_func_t some_func = (some_func_t)dlsym(handle, "rustdesk_core_main");
    const char *dlsym_error = dlerror();
    if (dlsym_error)
    {
        std::cerr << "Cannot load symbol 'some_func': " << dlsym_error << '\n';
        dlclose(handle);
        return 1;
    }

    some_func();

    dlclose(handle);
}
