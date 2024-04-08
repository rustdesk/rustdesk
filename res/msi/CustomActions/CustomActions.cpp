// CustomAction.cpp : Defines the entry point for the custom action.
#include "pch.h"
#include <strutil.h>
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
    LPWSTR installFolder = NULL;
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;

    hr = WcaInitialize(hInstall, "RemoveInstallFolder");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &installFolder);
    ExitOnFailure(hr, "failed to read database key from custom action data: %ls", pwz);

    SHFILEOPSTRUCTW fileOp;
    ZeroMemory(&fileOp, sizeof(SHFILEOPSTRUCT)); 

    fileOp.wFunc = FO_DELETE;
    fileOp.pFrom = installFolder;
    fileOp.fFlags = FOF_NOCONFIRMATION | FOF_SILENT;

    nResult = SHFileOperation(&fileOp);
    if (nResult == 0)
    {
        WcaLog(LOGMSG_STANDARD, "The directory \"%ls\" has been deleted.", installFolder);
    }
    else
    {
        WcaLog(LOGMSG_STANDARD, "The directory \"%ls\" has not been deleted, error code: 0X%02X. Please refer to https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shfileoperationa for the error codes.", installFolder, nResult);
    }

LExit:
    ReleaseStr(installFolder);

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}
