// CustomAction.cpp : Defines the entry point for the custom action.
#include "pch.h"
#include <stdlib.h>
#include <strutil.h>
#include <shellapi.h>
#include <tlhelp32.h>
#include <winternl.h>
#include <netfw.h>
#include <shlwapi.h>

#include "./Common.h"

#pragma comment(lib, "Shlwapi.lib")

UINT __stdcall CustomActionHello(
    __in MSIHANDLE hInstall)
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

// Helper function to safely delete a file or directory using handle-based deletion.
// This avoids TOCTOU (Time-Of-Check-Time-Of-Use) race conditions.
BOOL SafeDeleteItem(LPCWSTR fullPath)
{
    // Open the file/directory with DELETE access and FILE_FLAG_OPEN_REPARSE_POINT
    // to prevent following symlinks.
    // Use shared access to allow deletion even when other processes have the file open.
    DWORD flags = FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT;
    HANDLE hFile = CreateFileW(
        fullPath,
        DELETE,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,  // Allow shared access
        NULL,
        OPEN_EXISTING,
        flags,
        NULL
    );

    if (hFile == INVALID_HANDLE_VALUE)
    {
        WcaLog(LOGMSG_STANDARD, "SafeDeleteItem: Failed to open '%ls'. Error: %lu", fullPath, GetLastError());
        return FALSE;
    }

    // Use SetFileInformationByHandle to mark for deletion.
    // The file will be deleted when the handle is closed.
    FILE_DISPOSITION_INFO dispInfo;
    dispInfo.DeleteFile = TRUE;

    BOOL result = SetFileInformationByHandle(
        hFile,
        FileDispositionInfo,
        &dispInfo,
        sizeof(dispInfo)
    );

    if (!result)
    {
        DWORD error = GetLastError();
        WcaLog(LOGMSG_STANDARD, "SafeDeleteItem: Failed to mark '%ls' for deletion. Error: %lu", fullPath, error);
    }

    CloseHandle(hFile);
    return result;
}

// Helper function to recursively delete a directory's contents with detailed logging.
void RecursiveDelete(LPCWSTR path)
{
    // Ensure the path is not empty or null.
    if (path == NULL || path[0] == L'\0')
    {
        return;
    }

    // Extra safety: never operate directly on a root path.
    if (PathIsRootW(path))
    {
        WcaLog(LOGMSG_STANDARD, "RecursiveDelete: refusing to operate on root path '%ls'.", path);
        return;
    }

    // MAX_PATH is enough here since the installer should not be using longer paths.
    // No need to handle extended-length paths (\\?\) in this context.
    WCHAR searchPath[MAX_PATH];
    HRESULT hr = StringCchPrintfW(searchPath, MAX_PATH, L"%s\\*", path);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "RecursiveDelete: Path too long to enumerate: %ls", path);
        return;
    }

    WIN32_FIND_DATAW findData;
    HANDLE hFind = FindFirstFileW(searchPath, &findData);

    if (hFind == INVALID_HANDLE_VALUE)
    {
        // This can happen if the directory is empty or doesn't exist, which is not an error in our case.
        WcaLog(LOGMSG_STANDARD, "RecursiveDelete: Failed to enumerate directory '%ls'. It may be missing or inaccessible. Error: %lu", path, GetLastError());
        return;
    }

    do
    {
        // Skip '.' and '..' directories.
        if (wcscmp(findData.cFileName, L".") == 0 || wcscmp(findData.cFileName, L"..") == 0)
        {
            continue;
        }

        // MAX_PATH is enough here since the installer should not be using longer paths.
        // No need to handle extended-length paths (\\?\) in this context.
        WCHAR fullPath[MAX_PATH];
        hr = StringCchPrintfW(fullPath, MAX_PATH, L"%s\\%s", path, findData.cFileName);
        if (FAILED(hr)) {
            WcaLog(LOGMSG_STANDARD, "RecursiveDelete: Path too long for item '%ls' in '%ls', skipping.", findData.cFileName, path);
            continue;
        }

        // Before acting, ensure the read-only attribute is not set.
        if (findData.dwFileAttributes & FILE_ATTRIBUTE_READONLY)
        {
            if (FALSE == SetFileAttributesW(fullPath, findData.dwFileAttributes & ~FILE_ATTRIBUTE_READONLY))
            {
                WcaLog(LOGMSG_STANDARD, "RecursiveDelete: Failed to remove read-only attribute. Error: %lu", GetLastError());
            }
        }

        if (findData.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY)
        {
            // Check for reparse points (symlinks/junctions) to prevent directory traversal attacks.
            // Do not follow reparse points, only remove the link itself.
            if (findData.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT)
            {
                WcaLog(LOGMSG_STANDARD, "RecursiveDelete: Not recursing into reparse point (symlink/junction), deleting link itself: %ls", fullPath);
                SafeDeleteItem(fullPath);
            }
            else
            {
                // Recursively delete directory contents first
                RecursiveDelete(fullPath);
                // Then delete the directory itself
                SafeDeleteItem(fullPath);
            }
        }
        else
        {
            // Delete file using safe handle-based deletion
            SafeDeleteItem(fullPath);
        }
    } while (FindNextFileW(hFind, &findData) != 0);

    DWORD lastError = GetLastError();
    if (lastError != ERROR_NO_MORE_FILES)
    {
        WcaLog(LOGMSG_STANDARD, "RecursiveDelete: FindNextFileW failed with error %lu", lastError);
    }

    FindClose(hFind);
}

// See `Package.wxs` for the sequence of this custom action.
//
// Upgrade/uninstall sequence:
//   1. InstallInitialize
//   2. RemoveExistingProducts
//      ├─ TerminateProcesses
//      ├─ TryStopDeleteService
//      ├─ RemoveInstallFolder - <-- Here
//      └─ RemoveFiles
//   3. InstallValidate
//   4. InstallFiles
//   5. InstallExecute
//   6. InstallFinalize
UINT __stdcall RemoveInstallFolder(
    __in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

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

    if (installFolder == NULL || installFolder[0] == L'\0') {
        WcaLog(LOGMSG_STANDARD, "Install folder path is empty, skipping recursive delete.");
        goto LExit;
    }

    if (PathIsRootW(installFolder)) {
        WcaLog(LOGMSG_STANDARD, "Refusing to recursively delete root folder '%ls'.", installFolder);
        goto LExit;
    }

    WcaLog(LOGMSG_STANDARD, "Attempting to recursively delete contents of install folder: %ls", installFolder);

    RecursiveDelete(installFolder);

    // The standard MSI 'RemoveFolders' action will take care of removing the (now empty) directories.
    // We don't need to call RemoveDirectoryW on installFolder itself, as it might still be in use by the installer.

LExit:
    ReleaseStr(pwzData);

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

// https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntqueryinformationprocess
// **NtQueryInformationProcess** may be altered or unavailable in future versions of Windows.
// Applications should use the alternate functions listed in this topic.
// But I do not find the alternate functions.
// https://github.com/heim-rs/heim/issues/105#issuecomment-683647573
typedef NTSTATUS(NTAPI *pfnNtQueryInformationProcess)(HANDLE, PROCESSINFOCLASS, PVOID, ULONG, PULONG);
bool TerminateProcessIfNotContainsParam(pfnNtQueryInformationProcess NtQueryInformationProcess, HANDLE process, LPCWSTR excludeParam)
{
    bool processClosed = false;
    PROCESS_BASIC_INFORMATION processInfo;
    NTSTATUS status = NtQueryInformationProcess(process, ProcessBasicInformation, &processInfo, sizeof(processInfo), NULL);
    if (status == 0 && processInfo.PebBaseAddress != NULL)
    {
        PEB peb;
        SIZE_T dwBytesRead;
        if (ReadProcessMemory(process, processInfo.PebBaseAddress, &peb, sizeof(peb), &dwBytesRead))
        {
            RTL_USER_PROCESS_PARAMETERS pebUpp;
            if (ReadProcessMemory(process,
                                  peb.ProcessParameters,
                                  &pebUpp,
                                  sizeof(RTL_USER_PROCESS_PARAMETERS),
                                  &dwBytesRead))
            {
                if (pebUpp.CommandLine.Length > 0)
                {
                    // Allocate extra space for null terminator
                    WCHAR *commandLine = (WCHAR *)malloc(pebUpp.CommandLine.Length + sizeof(WCHAR));
                    if (commandLine != NULL)
                    {
                        // Initialize all bytes to zero for safety
                        memset(commandLine, 0, pebUpp.CommandLine.Length + sizeof(WCHAR));
                        if (ReadProcessMemory(process, pebUpp.CommandLine.Buffer,
                                              commandLine, pebUpp.CommandLine.Length, &dwBytesRead))
                        {
                            if (wcsstr(commandLine, excludeParam) == NULL)
                            {
                                WcaLog(LOGMSG_STANDARD, "Terminate process : %ls", commandLine);
                                TerminateProcess(process, 0);
                                processClosed = true;
                            }
                        }
                        free(commandLine);
                    }
                }
            }
        }
    }
    return processClosed;
}

// Terminate processes that do not have parameter [excludeParam]
// Note. This function relies on "NtQueryInformationProcess",
//       which may not be found.
//       Then all processes of [processName] will be terminated.
bool TerminateProcessesByNameW(LPCWSTR processName, LPCWSTR excludeParam)
{
    HMODULE hntdll = GetModuleHandleW(L"ntdll.dll");
    if (hntdll == NULL)
    {
        WcaLog(LOGMSG_STANDARD, "Failed to load ntdll.");
    }

    pfnNtQueryInformationProcess NtQueryInformationProcess = NULL;
    if (hntdll != NULL)
    {
        NtQueryInformationProcess = (pfnNtQueryInformationProcess)GetProcAddress(
            hntdll, "NtQueryInformationProcess");
    }
    if (NtQueryInformationProcess == NULL)
    {
        WcaLog(LOGMSG_STANDARD, "Failed to get address of NtQueryInformationProcess.");
    }

    bool processClosed = false;
    // Create a snapshot of the current system processes
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snapshot != INVALID_HANDLE_VALUE)
    {
        PROCESSENTRY32W processEntry;
        processEntry.dwSize = sizeof(PROCESSENTRY32W);
        if (Process32FirstW(snapshot, &processEntry))
        {
            do
            {
                if (lstrcmpW(processName, processEntry.szExeFile) == 0)
                {
                    HANDLE process = OpenProcess(PROCESS_TERMINATE | PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, processEntry.th32ProcessID);
                    if (process != NULL)
                    {
                        if (NtQueryInformationProcess == NULL)
                        {
                            WcaLog(LOGMSG_STANDARD, "Terminate process : %ls, while NtQueryInformationProcess is NULL", processName);
                            TerminateProcess(process, 0);
                            processClosed = true;
                        }
                        else
                        {
                            processClosed = TerminateProcessIfNotContainsParam(
                                NtQueryInformationProcess,
                                process,
                                excludeParam);
                        }
                        CloseHandle(process);
                    }
                }
            } while (Process32NextW(snapshot, &processEntry));
        }
        CloseHandle(snapshot);
    }
    if (hntdll != NULL)
    {
        CloseHandle(hntdll);
    }
    return processClosed;
}

UINT __stdcall TerminateProcesses(
    __in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    int nResult = 0;
    wchar_t szProcess[256] = {0};
    DWORD cchProcess = sizeof(szProcess) / sizeof(szProcess[0]);

    hr = WcaInitialize(hInstall, "TerminateProcesses");
    ExitOnFailure(hr, "Failed to initialize");

    MsiGetPropertyW(hInstall, L"TerminateProcesses", szProcess, &cchProcess);

    WcaLog(LOGMSG_STANDARD, "Try terminate processes : %ls", szProcess);
    TerminateProcessesByNameW(szProcess, L"--install");

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

// No use for now, it can be refer as an example of ShellExecuteW.
void AddFirewallRuleCmdline(LPWSTR exeName, LPWSTR exeFile, LPCWSTR dir)
{
    HRESULT hr = S_OK;
    HINSTANCE hi = 0;
    WCHAR cmdline[1024] = { 0, };
    WCHAR rulename[500] = { 0, };

    StringCchPrintfW(rulename, sizeof(rulename) / sizeof(rulename[0]), L"%ls Service", exeName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make rulename: %ls", exeName);
        return;
    }

    StringCchPrintfW(cmdline, sizeof(cmdline) / sizeof(cmdline[0]), L"advfirewall firewall add rule name=\"%ls\" dir=%ls action=allow program=\"%ls\" enable=yes", rulename, dir, exeFile);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make cmdline: %ls", exeName);
        return;
    }

    hi = ShellExecuteW(NULL, L"open", L"netsh", cmdline, NULL, SW_HIDE);
    // https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to change firewall rule : %d, last error: %d", (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Firewall rule \"%ls\" (%ls) is added", rulename, dir);
    }
}

// No use for now, it can be refer as an example of ShellExecuteW.
void RemoveFirewallRuleCmdline(LPWSTR exeName)
{
    HRESULT hr = S_OK;
    HINSTANCE hi = 0;
    WCHAR cmdline[1024] = { 0, };
    WCHAR rulename[500] = { 0, };

    StringCchPrintfW(rulename, sizeof(rulename) / sizeof(rulename[0]), L"%ls Service", exeName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make rulename: %ls", exeName);
        return;
    }

    StringCchPrintfW(cmdline, sizeof(cmdline) / sizeof(cmdline[0]), L"advfirewall firewall delete rule name=\"%ls\"", rulename);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make cmdline: %ls", exeName);
        return;
    }

    hi = ShellExecuteW(NULL, L"open", L"netsh", cmdline, NULL, SW_HIDE);
    // https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to change firewall rule \"%ls\" : %d, last error: %d", rulename, (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Firewall rule \"%ls\" is removed", rulename);
    }
}

UINT __stdcall AddFirewallRules(
    __in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    int nResult = 0;
    LPWSTR exeFile = NULL;
    LPWSTR exeName = NULL;
    WCHAR exeNameNoExt[500] = { 0, };
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;
    size_t szNameLen = 0;

    hr = WcaInitialize(hInstall, "AddFirewallRules");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &exeFile);
    ExitOnFailure(hr, "failed to read database key from custom action data: %ls", pwz);
    WcaLog(LOGMSG_STANDARD, "Try add firewall exceptions for file : %ls", exeFile);

    exeName = PathFindFileNameW(exeFile + 1);
    hr = StringCchPrintfW(exeNameNoExt, 500, exeName);
    ExitOnFailure(hr, "Failed to copy exe name: %ls", exeName);
    szNameLen = wcslen(exeNameNoExt);
    if (szNameLen >= 4 && wcscmp(exeNameNoExt + szNameLen - 4, L".exe") == 0) {
        exeNameNoExt[szNameLen - 4] = L'\0';
    }

    //if (exeFile[0] == L'1') {
    //    AddFirewallRuleCmdline(exeNameNoExt, exeFile, L"in");
    //    AddFirewallRuleCmdline(exeNameNoExt, exeFile, L"out");
    //}
    //else {
    //    RemoveFirewallRuleCmdline(exeNameNoExt);
    //}

    AddFirewallRule(exeFile[0] == L'1', exeNameNoExt, exeFile + 1);

LExit:
    if (pwzData) {
        ReleaseStr(pwzData);
    }

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall SetPropertyIsServiceRunning(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    wchar_t szAppName[500] = { 0 };
    DWORD cchAppName = sizeof(szAppName) / sizeof(szAppName[0]);
    wchar_t szPropertyName[500] = { 0 };
    DWORD cchPropertyName = sizeof(szPropertyName) / sizeof(szPropertyName[0]);
    bool isRunning = false;

    hr = WcaInitialize(hInstall, "SetPropertyIsServiceRunning");
    ExitOnFailure(hr, "Failed to initialize");

    MsiGetPropertyW(hInstall, L"AppName", szAppName, &cchAppName);
    WcaLog(LOGMSG_STANDARD, "Try query service of : \"%ls\"", szAppName);

    MsiGetPropertyW(hInstall, L"PropertyName", szPropertyName, &cchPropertyName);
    WcaLog(LOGMSG_STANDARD, "Try set is service running, property name : \"%ls\"", szPropertyName);

    isRunning = IsServiceRunningW(szAppName);
    MsiSetPropertyW(hInstall, szPropertyName, isRunning ? L"'N'" : L"'Y'");

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

void TryCreateStartServiceByShell(LPWSTR svcName, LPWSTR svcBinary, LPWSTR szSvcDisplayName);
UINT __stdcall CreateStartService(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    LPWSTR svcParams = NULL;
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;
    LPWSTR svcName = NULL;
    LPWSTR svcBinary = NULL;
    wchar_t szSvcDisplayName[500] = { 0 };
    DWORD cchSvcDisplayName = sizeof(szSvcDisplayName) / sizeof(szSvcDisplayName[0]);

    hr = WcaInitialize(hInstall, "CreateStartService");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &svcParams);
    ExitOnFailure(hr, "failed to read database key from custom action data: %ls", pwz);

    WcaLog(LOGMSG_STANDARD, "Try create start service : %ls", svcParams);

    svcName = svcParams;
    svcBinary = wcschr(svcParams, L';');
    if (svcBinary == NULL) {
        WcaLog(LOGMSG_STANDARD, "Failed to find binary : %ls", svcParams);
        goto LExit;
    }
    svcBinary[0] = L'\0';
    svcBinary += 1;

    hr = StringCchPrintfW(szSvcDisplayName, cchSvcDisplayName, L"%ls Service", svcName);
    ExitOnFailure(hr, "Failed to compose a resource identifier string");
    if (MyCreateServiceW(svcName, szSvcDisplayName, svcBinary)) {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is created.", svcName);
        if (MyStartServiceW(svcName)) {
            WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is started.", svcName);
        }
        else {
            WcaLog(LOGMSG_STANDARD, "Failed to start service: \"%ls\"", svcName);
        }
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Failed to create service: \"%ls\"", svcName);
    }

    if (IsServiceRunningW(svcName)) {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is running.", svcName);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is not running, try create and start service by shell", svcName);
        TryCreateStartServiceByShell(svcName, svcBinary, szSvcDisplayName);
    }

LExit:
    if (pwzData) {
        ReleaseStr(pwzData);
    }

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

void TryStopDeleteServiceByShell(LPWSTR svcName);
UINT __stdcall TryStopDeleteService(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    int nResult = 0;
    LPWSTR svcName = NULL;
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;
    wchar_t szExeFile[500] = { 0 };
    DWORD cchExeFile = sizeof(szExeFile) / sizeof(szExeFile[0]);
    SERVICE_STATUS_PROCESS svcStatus;
    DWORD lastErrorCode = 0;

    hr = WcaInitialize(hInstall, "TryStopDeleteService");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &svcName);
    ExitOnFailure(hr, "failed to read database key from custom action data: %ls", pwz);
    WcaLog(LOGMSG_STANDARD, "Try stop and delete service : %ls", svcName);

    if (MyStopServiceW(svcName)) {
        for (int i = 0; i < 10; i++) {
            if (IsServiceRunningW(svcName)) {
                Sleep(100);
            }
            else {
                break;
            }
        }
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Failed to stop service: \"%ls\", error: 0x%02X.", svcName, GetLastError());
    }

    if (IsServiceRunningW(svcName)) {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is not stoped after 1000 ms.", svcName);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is stoped.", svcName);
    }

    if (MyDeleteServiceW(svcName)) {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" deletion is completed without errors.", svcName);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Failed to delete service: \"%ls\", error: 0x%02X.", svcName, GetLastError());
    }

    if (QueryServiceStatusExW(svcName, &svcStatus)) {
        WcaLog(LOGMSG_STANDARD, "Failed to delete service: \"%ls\", current status: %d.", svcName, svcStatus.dwCurrentState);
        TryStopDeleteServiceByShell(svcName);
    }
    else {
        lastErrorCode = GetLastError();
        if (lastErrorCode == ERROR_SERVICE_DOES_NOT_EXIST) {
            WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is deleted.", svcName);
        }
        else {
            WcaLog(LOGMSG_STANDARD, "Failed to query service status: \"%ls\", error: 0x%02X.", svcName, lastErrorCode);
            TryStopDeleteServiceByShell(svcName);
        }
    }

    // It's really strange that we need sleep here.
    // But the upgrading may be stucked at "copying new files" because the file is in using.
    // Steps to reproduce: Install -> stop service in tray --> start service -> upgrade
    // Sleep(300);

    // Or we can terminate the process
    hr = StringCchPrintfW(szExeFile, cchExeFile, L"%ls.exe", svcName);
    ExitOnFailure(hr, "Failed to compose a resource identifier string");
    TerminateProcessesByNameW(szExeFile, L"--not-in-use");

LExit:
    if (pwzData) {
        ReleaseStr(pwzData);
    }

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall TryDeleteStartupShortcut(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    wchar_t szShortcut[500] = { 0 };
    DWORD cchShortcut = sizeof(szShortcut) / sizeof(szShortcut[0]);
    wchar_t szStartupDir[500] = { 0 };
    DWORD cchStartupDir = sizeof(szStartupDir) / sizeof(szStartupDir[0]);
    WCHAR pwszTemp[1024] = L"";

    hr = WcaInitialize(hInstall, "DeleteStartupShortcut");
    ExitOnFailure(hr, "Failed to initialize");

    MsiGetPropertyW(hInstall, L"StartupFolder", szStartupDir, &cchStartupDir);

    MsiGetPropertyW(hInstall, L"ShortcutName", szShortcut, &cchShortcut);
    WcaLog(LOGMSG_STANDARD, "Try delete startup shortcut of : \"%ls\"", szShortcut);

    hr = StringCchPrintfW(pwszTemp, 1024, L"%ls%ls.lnk", szStartupDir, szShortcut);
    ExitOnFailure(hr, "Failed to compose a resource identifier string");

    if (DeleteFileW(pwszTemp)) {
        WcaLog(LOGMSG_STANDARD, "Failed to delete startup shortcut of : \"%ls\"", pwszTemp);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Startup shortcut is deleted : \"%ls\"", pwszTemp);
    }

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall SetPropertyFromConfig(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    wchar_t szConfigFile[1024] = { 0 };
    DWORD cchConfigFile = sizeof(szConfigFile) / sizeof(szConfigFile[0]);
    wchar_t szConfigKey[500] = { 0 };
    DWORD cchConfigKey = sizeof(szConfigKey) / sizeof(szConfigKey[0]);
    wchar_t szPropertyName[500] = { 0 };
    DWORD cchPropertyName = sizeof(szPropertyName) / sizeof(szPropertyName[0]);
    std::wstring configValue;

    hr = WcaInitialize(hInstall, "SetPropertyFromConfig");
    ExitOnFailure(hr, "Failed to initialize");

    MsiGetPropertyW(hInstall, L"ConfigFile", szConfigFile, &cchConfigFile);
    WcaLog(LOGMSG_STANDARD, "Try read config file of : \"%ls\"", szConfigFile);

    MsiGetPropertyW(hInstall, L"ConfigKey", szConfigKey, &cchConfigKey);
    WcaLog(LOGMSG_STANDARD, "Try read configuration, config key : \"%ls\"", szConfigKey);

    MsiGetPropertyW(hInstall, L"PropertyName", szPropertyName, &cchPropertyName);
    WcaLog(LOGMSG_STANDARD, "Try read configuration, property name : \"%ls\"", szPropertyName);

    configValue = ReadConfig(szConfigFile, szConfigKey);
    MsiSetPropertyW(hInstall, szPropertyName, configValue.c_str());

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall AddRegSoftwareSASGeneration(__in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

     LSTATUS result = 0;
     HKEY hKey;
     LPCWSTR subKey = L"Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System";
     LPCWSTR valueName = L"SoftwareSASGeneration";
     DWORD valueType = REG_DWORD;
     DWORD valueData = 1;
     DWORD valueDataSize = sizeof(DWORD);

    HINSTANCE hi = 0;

    hr = WcaInitialize(hInstall, "AddRegSoftwareSASGeneration");
    ExitOnFailure(hr, "Failed to initialize");

    hi = ShellExecuteW(NULL, L"open", L"reg", L" add HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System /f /v SoftwareSASGeneration /t REG_DWORD /d 1", NULL, SW_HIDE);
    // https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to add registry name \"%ls\", %d, %d", valueName, (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Registry name \"%ls\" is added", valueName);
    }

    // Why RegSetValueExW always return 998?
    // 
    result = RegCreateKeyExW(HKEY_LOCAL_MACHINE, subKey, 0, NULL, REG_OPTION_NON_VOLATILE, KEY_WRITE, NULL, &hKey, NULL);
    if (result != ERROR_SUCCESS) {
        WcaLog(LOGMSG_STANDARD, "Failed to create or open registry key: %d", result);
        goto LExit;
    }

    result = RegSetValueExW(hKey, valueName, 0, valueType, reinterpret_cast<const BYTE*>(valueData), valueDataSize);
    if (result != ERROR_SUCCESS) {
        WcaLog(LOGMSG_STANDARD, "Failed to set registry value: %d", result);
        RegCloseKey(hKey);
        goto LExit;
    }

    WcaLog(LOGMSG_STANDARD, "Registry value has been successfully set.");
    RegCloseKey(hKey);

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall RemoveAmyuniIdd(
    __in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    int nResult = 0;
    LPWSTR installFolder = NULL;
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;

    WCHAR workDir[1024] = L"";
    DWORD fileAttributes = 0;
    HINSTANCE hi = 0;

    SYSTEM_INFO si;
    LPCWSTR exe = L"deviceinstaller64.exe";
    WCHAR exePath[1024] = L"";

    BOOL rebootRequired = FALSE;

    hr = WcaInitialize(hInstall, "RemoveAmyuniIdd");
    ExitOnFailure(hr, "Failed to initialize");

    UninstallDriver(L"usbmmidd", rebootRequired);

    // Only for x86 app on x64
    GetNativeSystemInfo(&si);
    if (si.wProcessorArchitecture != PROCESSOR_ARCHITECTURE_AMD64) {
        goto LExit;
    }

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &installFolder);
    ExitOnFailure(hr, "failed to read database key from custom action data: %ls", pwz);

    hr = StringCchPrintfW(workDir, 1024, L"%lsusbmmidd_v2", installFolder);
    ExitOnFailure(hr, "Failed to compose a resource identifier string");
    fileAttributes = GetFileAttributesW(workDir);
    if (fileAttributes == INVALID_FILE_ATTRIBUTES) {
        WcaLog(LOGMSG_STANDARD, "Amyuni idd dir \"%ls\" is not found, %d", workDir, fileAttributes);
        goto LExit;
    }

    hr = StringCchPrintfW(exePath, 1024, L"%ls\\%ls", workDir, exe);
    ExitOnFailure(hr, "Failed to compose a resource identifier string");
    fileAttributes = GetFileAttributesW(exePath);
    if (fileAttributes == INVALID_FILE_ATTRIBUTES) {
        goto LExit;
    }

    WcaLog(LOGMSG_STANDARD, "Remove amyuni idd %ls in %ls", exe, workDir);
    hi = ShellExecuteW(NULL, L"open", exe, L"remove usbmmidd", workDir, SW_HIDE);
    // https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to remove amyuni idd : %d, last error: %d", (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Amyuni idd is removed");
    }

LExit:
    if (pwzData) {
        ReleaseStr(pwzData);
    }

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

void TryCreateStartServiceByShell(LPWSTR svcName, LPWSTR svcBinary, LPWSTR szSvcDisplayName)
{
    HRESULT hr = S_OK;
    HINSTANCE hi = 0;
    wchar_t szNewBin[500] = { 0 };
    DWORD cchNewBin = sizeof(szNewBin) / sizeof(szNewBin[0]);
    wchar_t szCmd[800] = { 0 };
    DWORD cchCmd = sizeof(szCmd) / sizeof(szCmd[0]);
    SERVICE_STATUS_PROCESS svcStatus;
    DWORD lastErrorCode = 0;
    int i = 0;
    int j = 0;

    WcaLog(LOGMSG_STANDARD, "TryCreateStartServiceByShell, service: %ls", svcName);

    TryStopDeleteServiceByShell(svcName);
    // Do not check the result here

    i = 0;
    j = 0;
    // svcBinary is a string with double quotes, we need to escape it for shell arguments.
    // It is orignal used for `CreateServiceW`.
    // eg. "C:\Program Files\MyApp\MyApp.exe" --service -> \"C:\Program Files\MyApp\MyApp.exe\" --service
    while (true) {
        if (svcBinary[j] == L'"') {
            szNewBin[i] = L'\\';
            i += 1;
            if (i >= cchNewBin) {
                WcaLog(LOGMSG_STANDARD, "Failed to copy bin for service: %ls, buffer is not enough", svcName);
                return;
            }
            szNewBin[i] = L'"';
        }
        else {
            szNewBin[i] = svcBinary[j];
        }
        if (svcBinary[j] == L'\0') {
            break;
        }
        i += 1;
        j += 1;
        if (i >= cchNewBin) {
            WcaLog(LOGMSG_STANDARD, "Failed to copy bin for service: %ls, buffer is not enough", svcName);
            return;
        }
    }

    hr = StringCchPrintfW(szCmd, cchCmd, L"create %ls binpath= \"%ls\" start= auto DisplayName= \"%ls\"", svcName, szNewBin, szSvcDisplayName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make command: %ls", svcName);
        return;
    }
    hi = ShellExecuteW(NULL, L"open", L"sc", szCmd, NULL, SW_HIDE);
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to create service with shell : %d, last error: 0x%02X.", (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is created with shell.", svcName);
    }

    // Query and log if the service is running.
    for (int k = 0; k < 10; ++k) {
        if (!QueryServiceStatusExW(svcName, &svcStatus)) {
            lastErrorCode = GetLastError();
            if (lastErrorCode == ERROR_SERVICE_DOES_NOT_EXIST) {
                if (k == 29) {
                    WcaLog(LOGMSG_STANDARD, "Failed to query service status: \"%ls\", service is not found.", svcName);
                    return;
                }
                else {
                    Sleep(100);
                    continue;
                }
            }
            // Break if the service exists.
            WcaLog(LOGMSG_STANDARD, "Failed to query service status: \"%ls\", error: 0x%02X.", svcName, lastErrorCode);
            break;
        }
        else {
            if (svcStatus.dwCurrentState == SERVICE_RUNNING) {
                WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is running.", svcName);
                return;
            }
            WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is not running.", svcName);
            break;
        }
    }

    hr = StringCchPrintfW(szCmd, cchCmd, L"/c sc start %ls", svcName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make command: %ls", svcName);
        return;
    }
    hi = ShellExecuteW(NULL, L"open", L"cmd.exe", szCmd, NULL, SW_HIDE);
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to start service with shell : %d, last error: 0x%02X.", (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is started with shell.", svcName);
    }
}

void TryStopDeleteServiceByShell(LPWSTR svcName)
{
    HRESULT hr = S_OK;
    HINSTANCE hi = 0;
    wchar_t szCmd[800] = { 0 };
    DWORD cchCmd = sizeof(szCmd) / sizeof(szCmd[0]);
    SERVICE_STATUS_PROCESS svcStatus;
    DWORD lastErrorCode = 0;

    WcaLog(LOGMSG_STANDARD, "TryStopDeleteServiceByShell, service: %ls", svcName);

    hr = StringCchPrintfW(szCmd, cchCmd, L"/c sc stop %ls", svcName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make command: %ls", svcName);
        return;
    }
    hi = ShellExecuteW(NULL, L"open", L"cmd.exe", szCmd, NULL, SW_HIDE);

    // Query and log if the service is stopped or deleted.
    for (int k = 0; k < 10; ++k) {
        if (!IsServiceRunningW(svcName)) {
            break;
        }
        Sleep(100);
    }
    if (!QueryServiceStatusExW(svcName, &svcStatus)) {
        if (GetLastError() == ERROR_SERVICE_DOES_NOT_EXIST) {
            WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is already deleted.", svcName);
            return;
        }
        WcaLog(LOGMSG_STANDARD, "Failed to query service status: \"%ls\" with shell, error: 0x%02X.", svcName, lastErrorCode);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Status of service: \"%ls\" with shell, current status: %d.", svcName, svcStatus.dwCurrentState);
    }

    hr = StringCchPrintfW(szCmd, cchCmd, L"/c sc delete %ls", svcName);
    if (FAILED(hr)) {
        WcaLog(LOGMSG_STANDARD, "Failed to make command: %ls", svcName);
        return;
    }
    hi = ShellExecuteW(NULL, L"open", L"cmd.exe", szCmd, NULL, SW_HIDE);
    if ((int)hi <= 32) {
        WcaLog(LOGMSG_STANDARD, "Failed to delete service with shell : %d, last error: 0x%02X.", (int)hi, GetLastError());
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Service \"%ls\" deletion is completed without errors with shell,", svcName);
    }

    // Query and log the status of the service after deletion.
    for (int k = 0; k < 10; ++k) {
        if (!QueryServiceStatusExW(svcName, &svcStatus)) {
            if (GetLastError() == ERROR_SERVICE_DOES_NOT_EXIST) {
                WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is deleted with shell.", svcName);
                return;
            }
        }
        Sleep(100);
    }
    if (!QueryServiceStatusExW(svcName, &svcStatus)) {
        lastErrorCode = GetLastError();
        if (lastErrorCode == ERROR_SERVICE_DOES_NOT_EXIST) {
            WcaLog(LOGMSG_STANDARD, "Service \"%ls\" is deleted with shell.", svcName);
            return;
        }
        WcaLog(LOGMSG_STANDARD, "Failed to query service status: \"%ls\" with shell, error: 0x%02X.", svcName, lastErrorCode);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "Failed to delete service: \"%ls\" with shell, current status: %d.", svcName, svcStatus.dwCurrentState);
    }
}

UINT __stdcall InstallPrinter(
    __in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    int nResult = 0;
    LPWSTR installFolder = NULL;
    LPWSTR pwz = NULL;
    LPWSTR pwzData = NULL;

    hr = WcaInitialize(hInstall, "InstallPrinter");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    pwz = pwzData;
    hr = WcaReadStringFromCaData(&pwz, &installFolder);
    ExitOnFailure(hr, "failed to read database key from custom action data: %ls", pwz);

    WcaLog(LOGMSG_STANDARD, "Try to install RD printer in : %ls", installFolder);
    RemotePrinter::installUpdatePrinter(installFolder);
    WcaLog(LOGMSG_STANDARD, "Install RD printer done");

LExit:
    if (pwzData) {
        ReleaseStr(pwzData);
    }

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}

UINT __stdcall UninstallPrinter(
    __in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    hr = WcaInitialize(hInstall, "UninstallPrinter");
    ExitOnFailure(hr, "Failed to initialize");

    WcaLog(LOGMSG_STANDARD, "Try to uninstall RD printer");
    RemotePrinter::uninstallPrinter();
    WcaLog(LOGMSG_STANDARD, "Uninstall RD printer done");

LExit:
    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}
