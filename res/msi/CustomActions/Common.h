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
