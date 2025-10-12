// dllmain.cpp : Defines the entry point for the DLL application.
#include "pch.h"

BOOL APIENTRY DllMain(
    __in HMODULE hModule,
    __in DWORD ulReasonForCall,
    __in LPVOID
)
{
    switch (ulReasonForCall)
    {
    case DLL_PROCESS_ATTACH:
        WcaGlobalInitialize(hModule);
        break;

    case DLL_PROCESS_DETACH:
        WcaGlobalFinalize();
        break;

    case DLL_THREAD_ATTACH:
    case DLL_THREAD_DETACH:
        break;
    }

    return TRUE;
}
