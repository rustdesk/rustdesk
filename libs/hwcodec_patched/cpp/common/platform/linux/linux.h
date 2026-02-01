#ifndef LINUX_H
#define LINUX_H

extern "C" int linux_support_nv();
extern "C" int linux_support_amd();
extern "C" int linux_support_intel();
extern "C" int setup_parent_death_signal();

#endif