#ifndef SYSTEM_H
#define SYSTEM_H

#ifdef _WIN32
#include "platform/win/win.h"
#endif
#ifdef __linux__
#include "platform/linux/linux.h"
#endif

#endif // SYSTEM_H