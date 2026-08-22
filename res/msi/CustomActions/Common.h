#pragma once

#include <Windows.h>
#include <string>

bool AddFirewallRule(bool add, LPWSTR exeName, LPWSTR exeFile);

bool QueryServiceStatusExW(LPCWSTR serviceName, SERVICE_STATUS_PROCESS* status);
bool IsServiceRunningW(LPCWSTR serviceName);
bool MyCreateServiceW(LPCWSTR serviceName, LPCWSTR displayName, LPCWSTR binaryPath);
bool MyDeleteServiceW(LPCWSTR serviceName);
bool MyStartServiceW(LPCWSTR serviceName);
bool MyStopServiceW(LPCWSTR serviceName);

std::wstring ReadConfig(const std::wstring& filename, const std::wstring& key);

void UninstallDriver(LPCWSTR hardwareId, BOOL &rebootRequired);

namespace RemotePrinter
{
    // `appName` names the printer and its port. It is passed in rather than compiled
    // in so that a single dll serves every custom client; an empty value keeps the
    // stock "RustDesk Printer" name.
    VOID installUpdatePrinter(const std::wstring& installFolder, const std::wstring& appName);
    VOID uninstallPrinter(const std::wstring& appName);
}
