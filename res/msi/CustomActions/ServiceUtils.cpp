// https://learn.microsoft.com/en-us/windows/win32/services/installing-a-service

#include "pch.h"

#include <iostream>
#include <Windows.h>
#include <strsafe.h>

bool MyCreateServiceW(LPCWSTR serviceName, LPCWSTR displayName, LPCWSTR binaryPath)
{
    SC_HANDLE schSCManager;
    SC_HANDLE schService;

    // Get a handle to the SCM database. 
    schSCManager = OpenSCManager(
        NULL,                    // local computer
        NULL,                    // ServicesActive database 
        SC_MANAGER_ALL_ACCESS);  // full access rights 

    if (NULL == schSCManager)
    {
        WcaLog(LOGMSG_STANDARD, "OpenSCManager failed (%d)\n", GetLastError());
        return false;
    }

    // Create the service
    schService = CreateService(
        schSCManager,              // SCM database 
        serviceName,               // name of service 
        displayName,               // service name to display 
        SERVICE_ALL_ACCESS,        // desired access 
        SERVICE_WIN32_OWN_PROCESS, // service type 
        SERVICE_AUTO_START,        // start type 
        SERVICE_ERROR_NORMAL,      // error control type 
        binaryPath,                // path to service's binary 
        NULL,                      // no load ordering group 
        NULL,                      // no tag identifier 
        NULL,                      // no dependencies 
        NULL,                      // LocalSystem account 
        NULL);                     // no password 
    if (schService == NULL)
    {
        WcaLog(LOGMSG_STANDARD, "CreateService failed (%d)\n", GetLastError());
        CloseServiceHandle(schSCManager);
        return false;
    }
    else
    {
        WcaLog(LOGMSG_STANDARD, "Service installed successfully\n");
    }

    CloseServiceHandle(schService);
    CloseServiceHandle(schSCManager);
    return true;
}

bool MyDeleteServiceW(LPCWSTR serviceName)
{
    SC_HANDLE hSCManager = OpenSCManagerW(NULL, NULL, SC_MANAGER_CONNECT);
    if (hSCManager == NULL) {
        WcaLog(LOGMSG_STANDARD, "Failed to open Service Control Manager");
        return false;
    }

    SC_HANDLE hService = OpenServiceW(hSCManager, serviceName, SERVICE_STOP | DELETE);
    if (hService == NULL) {
        WcaLog(LOGMSG_STANDARD, "Failed to open service: %ls", serviceName);
        CloseServiceHandle(hSCManager);
        return false;
    }

    SERVICE_STATUS serviceStatus;
    if (ControlService(hService, SERVICE_CONTROL_STOP, &serviceStatus)) {
        WcaLog(LOGMSG_STANDARD, "Stopping service: %ls", serviceName);
    }

    bool success = DeleteService(hService);
    if (!success) {
        WcaLog(LOGMSG_STANDARD, "Failed to delete service: %ls", serviceName);
    }

    CloseServiceHandle(hService);
    CloseServiceHandle(hSCManager);

    return success;
}

bool MyStartServiceW(LPCWSTR serviceName)
{
    SC_HANDLE hSCManager = OpenSCManagerW(NULL, NULL, SC_MANAGER_CONNECT);
    if (hSCManager == NULL) {
        WcaLog(LOGMSG_STANDARD, "Failed to open Service Control Manager");
        return false;
    }

    SC_HANDLE hService = OpenServiceW(hSCManager, serviceName, SERVICE_START);
    if (hService == NULL) {
        WcaLog(LOGMSG_STANDARD, "Failed to open service: %ls", serviceName);
        CloseServiceHandle(hSCManager);
        return false;
    }

    bool success = StartService(hService, 0, NULL);
    if (!success) {
        WcaLog(LOGMSG_STANDARD, "Failed to start service: %ls", serviceName);
    }

    CloseServiceHandle(hService);
    CloseServiceHandle(hSCManager);

    return success;
}

bool MyStopServiceW(LPCWSTR serviceName)
{
    SC_HANDLE hSCManager = OpenSCManagerW(NULL, NULL, SC_MANAGER_CONNECT);
    if (hSCManager == NULL) {
        WcaLog(LOGMSG_STANDARD, "Failed to open Service Control Manager");
        return false;
    }

    SC_HANDLE hService = OpenServiceW(hSCManager, serviceName, SERVICE_STOP);
    if (hService == NULL) {
        WcaLog(LOGMSG_STANDARD, "Failed to open service: %ls", serviceName);
        CloseServiceHandle(hSCManager);
        return false;
    }

    SERVICE_STATUS serviceStatus;
    if (!ControlService(hService, SERVICE_CONTROL_STOP, &serviceStatus)) {
        WcaLog(LOGMSG_STANDARD, "Failed to stop service: %ls", serviceName);
        CloseServiceHandle(hService);
        CloseServiceHandle(hSCManager);
        return false;
    }

    CloseServiceHandle(hService);
    CloseServiceHandle(hSCManager);

    return true;
}

bool IsServiceRunningW(LPCWSTR serviceName)
{
    SC_HANDLE hSCManager = OpenSCManagerW(NULL, NULL, SC_MANAGER_CONNECT);
    if (hSCManager == NULL) {
        WcaLog(LOGMSG_STANDARD, "Failed to open Service Control Manager");
        return false;
    }

    SC_HANDLE hService = OpenServiceW(hSCManager, serviceName, SERVICE_QUERY_STATUS);
    if (hService == NULL) {
        WcaLog(LOGMSG_STANDARD, "Failed to open service: %ls", serviceName);
        CloseServiceHandle(hSCManager);
        return false;
    }

    SERVICE_STATUS_PROCESS serviceStatus;
    DWORD bytesNeeded;
    if (!QueryServiceStatusEx(hService, SC_STATUS_PROCESS_INFO, reinterpret_cast<LPBYTE>(&serviceStatus), sizeof(serviceStatus), &bytesNeeded)) {
        WcaLog(LOGMSG_STANDARD, "Failed to query service: %ls", serviceName);
        CloseServiceHandle(hService);
        CloseServiceHandle(hSCManager);
        return false;
    }

    bool isRunning = (serviceStatus.dwCurrentState == SERVICE_RUNNING);

    CloseServiceHandle(hService);
    CloseServiceHandle(hSCManager);

    return isRunning;
}
