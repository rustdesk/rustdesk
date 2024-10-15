#include "pch.h"

#include <Windows.h>
#include <setupapi.h>
#include <devguid.h>
#include <cfgmgr32.h>

#pragma comment(lib, "SetupAPI.lib")


void UninstallDriver(LPCWSTR hardwareId, BOOL &rebootRequired)
{
    HDEVINFO deviceInfoSet = SetupDiGetClassDevsW(&GUID_DEVCLASS_DISPLAY, NULL, NULL, DIGCF_PRESENT);
    if (deviceInfoSet == INVALID_HANDLE_VALUE)
    {
        WcaLog(LOGMSG_STANDARD, "Failed to get device information set, last error: %d", GetLastError());
        return;
    }

    SP_DEVINFO_LIST_DETAIL_DATA devInfoListDetail;
    devInfoListDetail.cbSize = sizeof(SP_DEVINFO_LIST_DETAIL_DATA);
    if (!SetupDiGetDeviceInfoListDetailW(deviceInfoSet, &devInfoListDetail))
    {
        SetupDiDestroyDeviceInfoList(deviceInfoSet);
        WcaLog(LOGMSG_STANDARD, "Failed to call SetupDiGetDeviceInfoListDetail, last error: %d", GetLastError());
        return;
    }

    SP_DEVINFO_DATA deviceInfoData;
    deviceInfoData.cbSize = sizeof(SP_DEVINFO_DATA);

    DWORD dataType;
    WCHAR deviceId[MAX_DEVICE_ID_LEN] = { 0, };

    DWORD deviceIndex = 0;
    while (SetupDiEnumDeviceInfo(deviceInfoSet, deviceIndex, &deviceInfoData))
    {
        if (!SetupDiGetDeviceRegistryPropertyW(deviceInfoSet, &deviceInfoData, SPDRP_HARDWAREID, &dataType, (PBYTE)deviceId, MAX_DEVICE_ID_LEN, NULL))
        {
            WcaLog(LOGMSG_STANDARD, "Failed to get hardware id, last error: %d", GetLastError());
            deviceIndex++;
            continue;
        }
        if (wcscmp(deviceId, hardwareId) != 0)
        {
            deviceIndex++;
            continue;
        }

        SP_REMOVEDEVICE_PARAMS remove_device_params;
        remove_device_params.ClassInstallHeader.cbSize = sizeof(SP_CLASSINSTALL_HEADER);
        remove_device_params.ClassInstallHeader.InstallFunction = DIF_REMOVE;
        remove_device_params.Scope = DI_REMOVEDEVICE_GLOBAL;
        remove_device_params.HwProfile = 0;

        if (!SetupDiSetClassInstallParamsW(deviceInfoSet, &deviceInfoData, &remove_device_params.ClassInstallHeader, sizeof(SP_REMOVEDEVICE_PARAMS)))
        {
            WcaLog(LOGMSG_STANDARD, "Failed to set class install params, last error: %d", GetLastError());
            deviceIndex++;
            continue;
        }

        if (!SetupDiCallClassInstaller(DIF_REMOVE, deviceInfoSet, &deviceInfoData))
        {
            WcaLog(LOGMSG_STANDARD, "ailed to uninstall driver, last error: %d", GetLastError());
            deviceIndex++;
            continue;
        }

        SP_DEVINSTALL_PARAMS deviceParams;
        if (SetupDiGetDeviceInstallParamsW(deviceInfoSet, &deviceInfoData, &deviceParams))
        {
            if (deviceParams.Flags & (DI_NEEDRESTART | DI_NEEDREBOOT))
            {
                rebootRequired = true;
            }
        }

        WcaLog(LOGMSG_STANDARD, "Driver uninstalled successfully");
        deviceIndex++;
    }

    SetupDiDestroyDeviceInfoList(deviceInfoSet);
}
