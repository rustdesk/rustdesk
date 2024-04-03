// CustomAction.cpp : Defines the entry point for the custom action.
#include "pch.h"
#include <shellapi.h>

UINT __stdcall CustomActionHello(
    __in MSIHANDLE hInstall
)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    hr = WcaInitialize(hInstall, "CustomActionHello");
    ExitOnFailure(hr, "Failed to initialize");

    WcaLog(LOGMSG_STANDARD, "Initialized.");

    // TODO: Add your custom action code here.
    WcaLog(LOGMSG_STANDARD, "================= Example CustomAction Hello");

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall RemoveInstallFolder(
    __in MSIHANDLE hInstall
)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    int nResult = 0;
    wchar_t szCustomActionData[256] = { 0 };
    DWORD cchCustomActionData = sizeof(szCustomActionData) / sizeof(szCustomActionData[0]);

    hr = WcaInitialize(hInstall, "RemoveInstallFolder");
    ExitOnFailure(hr, "Failed to initialize");

    MsiGetPropertyW(hInstall, L"InstallFolder", szCustomActionData, &cchCustomActionData);
    
    WcaLog(LOGMSG_STANDARD, "================= Remove Install Folder: %ls", szCustomActionData);

    SHFILEOPSTRUCTW fileOp;
    ZeroMemory(&fileOp, sizeof(SHFILEOPSTRUCT));

    fileOp.wFunc = FO_DELETE;
    fileOp.pFrom = szCustomActionData;
    fileOp.fFlags = FOF_NOCONFIRMATION | FOF_SILENT;

    nResult = SHFileOperationW(&fileOp);
    if (nResult == 0)
    {
        WcaLog(LOGMSG_STANDARD, "The directory \"%ls\" has been deleted.", szCustomActionData);
    }
    else
    {
        WcaLog(LOGMSG_STANDARD, "The directory \"%ls\" has not been deleted, error code: 0X%02X.", szCustomActionData, nResult);
    }

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}
