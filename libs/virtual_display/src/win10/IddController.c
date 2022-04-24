#include "./IddController.h"
#include <stdio.h>
#include <stdlib.h>
#include <newdev.h>
#include <swdevice.h>
#include <strsafe.h>
#include <cfgmgr32.h>
#include <combaseapi.h>

#include "./Public.h"


const GUID GUID_DEVINTERFACE_IDD_DRIVER_DEVICE = \
{ 0x781EF630, 0x72B2, 0x11d2, { 0xB8, 0x52,  0x00,  0xC0,  0x4E,  0xAF,  0x52,  0x72 } };
//{781EF630-72B2-11d2-B852-00C04EAF5272}

BOOL g_printMsg = TRUE;
char g_lastMsg[1024];
const char* g_msgHeader = "RustDeskIdd: ";

VOID WINAPI
CreationCallback(
    _In_ HSWDEVICE hSwDevice,
    _In_ HRESULT hrCreateResult,
    _In_opt_ PVOID pContext,
    _In_opt_ PCWSTR pszDeviceInstanceId
);
// https://github.com/microsoft/Windows-driver-samples/blob/9f03207ae1e8df83325f067de84494ae55ab5e97/general/DCHU/osrfx2_DCHU_base/osrfx2_DCHU_testapp/testapp.c#L88
// Not a good way for this device, I don't not why. I'm not familiar with dirver.
BOOLEAN GetDevicePath(
    _In_ LPCGUID InterfaceGuid,
    _Out_writes_(BufLen) PTCHAR DevicePath,
    _In_ size_t BufLen
);
// https://github.com/microsoft/Windows-driver-samples/blob/9f03207ae1e8df83325f067de84494ae55ab5e97/usb/umdf_fx2/exe/testapp.c#L90
// Works good to check whether device is created before.
BOOLEAN GetDevicePath2(
    _In_ LPCGUID InterfaceGuid,
    _Out_writes_(BufLen) PTCHAR DevicePath,
    _In_ size_t BufLen
);

HANDLE DeviceOpenHandle();
VOID DeviceCloseHandle(HANDLE handle);

void SetLastMsg(const char* format, ...)
{
    memset(g_lastMsg, 0, sizeof(g_lastMsg));
    memcpy_s(g_lastMsg, sizeof(g_lastMsg), g_msgHeader, strlen(g_msgHeader));

    va_list args;
    va_start(args, format);
    vsnprintf_s(
        g_lastMsg + strlen(g_msgHeader),
        sizeof(g_lastMsg) - strlen(g_msgHeader),
        _TRUNCATE,
        format,
        args);
    va_end(args);
}

const char* GetLastMsg()
{
    return g_lastMsg;
}

BOOL InstallUpdate(LPCWSTR fullInfPath, PBOOL rebootRequired)
{
    SetLastMsg("Sucess");

    // UpdateDriverForPlugAndPlayDevicesW may return FALSE while driver was successfully installed...
    if (FALSE == UpdateDriverForPlugAndPlayDevicesW(
        NULL,
        L"RustDeskIddDriver",    // match hardware id in the inf file
        fullInfPath,
        INSTALLFLAG_FORCE
            // | INSTALLFLAG_NONINTERACTIVE  // INSTALLFLAG_NONINTERACTIVE may cause error 0xe0000247
        ,
        rebootRequired
    ))
    {
        DWORD error = GetLastError();
        if (error != 0)
        {
            SetLastMsg("UpdateDriverForPlugAndPlayDevicesW failed, last error 0x%x\n", error);
            if (g_printMsg)
            {
                printf(g_lastMsg);
            }
            return FALSE;
        }
    }

    return TRUE;
}

BOOL Uninstall(LPCWSTR fullInfPath, PBOOL rebootRequired)
{
    SetLastMsg("Sucess");

    if (FALSE == DiUninstallDriverW(
        NULL,
        fullInfPath,
        0,
        rebootRequired
    ))
    {
        DWORD error = GetLastError();
        if (error != 0)
        {
            SetLastMsg("DiUninstallDriverW failed, last error 0x%x\n", error);
            if (g_printMsg)
            {
                printf(g_lastMsg);
            }
            return FALSE;
        }
    }

    return TRUE;
}

BOOL IsDeviceCreated(PBOOL created)
{
    SetLastMsg("Sucess");

    HDEVINFO hardwareDeviceInfo = SetupDiGetClassDevs(
        &GUID_DEVINTERFACE_IDD_DRIVER_DEVICE,
        NULL, // Define no enumerator (global)
        NULL, // Define no
        (DIGCF_PRESENT | // Only Devices present
            DIGCF_DEVICEINTERFACE)); // Function class devices.
    if (INVALID_HANDLE_VALUE == hardwareDeviceInfo)
    {
        SetLastMsg("Idd device: SetupDiGetClassDevs failed, last error 0x%x\n", GetLastError());
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        return FALSE;
    }

    SP_DEVICE_INTERFACE_DATA            deviceInterfaceData;
    deviceInterfaceData.cbSize = sizeof(SP_DEVICE_INTERFACE_DATA);

    BOOL ret = FALSE;
    do
    {
        if (TRUE == SetupDiEnumDeviceInterfaces(hardwareDeviceInfo,
            0, // No care about specific PDOs
            &GUID_DEVINTERFACE_IDD_DRIVER_DEVICE,
            0, //
            &deviceInterfaceData))
        {
            *created = TRUE;
            ret = TRUE;
            break;
        }

        DWORD error = GetLastError();
        if (error == ERROR_NO_MORE_ITEMS)
        {
            *created = FALSE;
            ret = TRUE;
            break;
        }

        SetLastMsg("Idd device: SetupDiEnumDeviceInterfaces failed, last error 0x%x\n", error);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        ret = FALSE;
        break;

    } while (0);

    (VOID)SetupDiDestroyDeviceInfoList(hardwareDeviceInfo);
    return ret;
}

BOOL DeviceCreate(PHSWDEVICE hSwDevice)
{
    SetLastMsg("Sucess");

    if (*hSwDevice != NULL)
    {
        SetLastMsg("Device handler is not NULL\n");
        return FALSE;
    }

    BOOL created = TRUE;
    if (FALSE == IsDeviceCreated(&created))
    {
        return FALSE;
    }
    if (created == TRUE)
    {
        SetLastMsg("Device is created before, please uninstall it first\n");
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        return FALSE;
    }

    // create device
    HANDLE hEvent = CreateEvent(NULL, FALSE, FALSE, NULL);
    if (hEvent == INVALID_HANDLE_VALUE || hEvent == NULL)
    {
        DWORD error = GetLastError();
        SetLastMsg("CreateEvent failed 0x%lx\n", error);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }

        return FALSE;
    }

    SW_DEVICE_CREATE_INFO createInfo = { 0 };
    PCWSTR description = L"RustDesk Idd Driver";

    // These match the Pnp id's in the inf file so OS will load the driver when the device is created    
    PCWSTR instanceId = L"RustDeskIddDriver";
    PCWSTR hardwareIds = L"RustDeskIddDriver\0\0";
    PCWSTR compatibleIds = L"RustDeskIddDriver\0\0";

    createInfo.cbSize = sizeof(createInfo);
    createInfo.pszzCompatibleIds = compatibleIds;
    createInfo.pszInstanceId = instanceId;
    createInfo.pszzHardwareIds = hardwareIds;
    createInfo.pszDeviceDescription = description;

    createInfo.CapabilityFlags = SWDeviceCapabilitiesRemovable |
        SWDeviceCapabilitiesSilentInstall |
        SWDeviceCapabilitiesDriverRequired;

    // Create the device
    HRESULT hr = SwDeviceCreate(L"RustDeskIddDriver",
        L"HTREE\\ROOT\\0",
        &createInfo,
        0,
        NULL,
        CreationCallback,
        &hEvent,
        hSwDevice);
    if (FAILED(hr))
    {
        SetLastMsg("SwDeviceCreate failed with 0x%lx\n", hr);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }

        return FALSE;
    }

    // Wait for callback to signal that the device has been created
    printf("Waiting for device to be created....\n");
    DWORD waitResult = WaitForSingleObject(hEvent, 10 * 1000);
    if (waitResult != WAIT_OBJECT_0)
    {
        SetLastMsg("Wait for device creation failed 0x%d\n", waitResult);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        return FALSE;
    }
    // printf("Device created\n\n");
    return TRUE;
}

VOID DeviceClose(HSWDEVICE hSwDevice)
{
    SetLastMsg("Sucess");

    if (hSwDevice != INVALID_HANDLE_VALUE && hSwDevice != NULL)
    {
        SwDeviceClose(hSwDevice);
    }
}

BOOL MonitorPlugIn(UINT index, UINT edid, INT retries)
{
    SetLastMsg("Sucess");

    if (retries < 0)
    {
        SetLastMsg("invalid tries %d\n", retries);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        return FALSE;
    }

    HANDLE hDevice = INVALID_HANDLE_VALUE;
    for (; retries >= 0; --retries)
    {
        hDevice = DeviceOpenHandle();
        if (hDevice != INVALID_HANDLE_VALUE && hDevice != NULL)
        {
            break;
        }
        Sleep(1000);
    }
    if (hDevice == INVALID_HANDLE_VALUE || hDevice == NULL)
    {
        return FALSE;
    }

    BOOL ret = FALSE;
    DWORD junk = 0;
    CtlPlugIn plugIn;
    plugIn.ConnectorIndex = index;
    plugIn.MonitorEDID = edid;
    HRESULT hr = CoCreateGuid(&plugIn.ContainerId);
    if (!SUCCEEDED(hr))
    {
        SetLastMsg("CoCreateGuid failed %d\n", hr);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        ret = FALSE;
    }
    else
    {
        ret = FALSE;
        for (; retries >= 0; --retries)
        {
            if (TRUE == DeviceIoControl(
                hDevice,
                IOCTL_CHANGER_IDD_PLUG_IN,
                &plugIn,                    // Ptr to InBuffer
                sizeof(CtlPlugIn),          // Length of InBuffer
                NULL,                       // Ptr to OutBuffer
                0,                          // Length of OutBuffer
                &junk,                      // BytesReturned
                0))                         // Ptr to Overlapped structure
            {
                ret = TRUE;
                break;
            }
        }
        if (ret == FALSE)
        {
            DWORD error = GetLastError();
            SetLastMsg("DeviceIoControl failed 0x%lx\n", error);
            printf(g_lastMsg);
        }
    }

    DeviceCloseHandle(hDevice);
    return ret;
}

BOOL MonitorPlugOut(UINT index)
{
    SetLastMsg("Sucess");

    HANDLE hDevice = DeviceOpenHandle();
    if (hDevice == INVALID_HANDLE_VALUE || hDevice == NULL)
    {
        return FALSE;
    }

    BOOL ret = FALSE;
    DWORD junk = 0;
    CtlPlugOut plugOut;
    plugOut.ConnectorIndex = index;
    if (!DeviceIoControl(
        hDevice,
        IOCTL_CHANGER_IDD_PLUG_OUT,
        &plugOut,               // Ptr to InBuffer
        sizeof(CtlPlugOut),     // Length of InBuffer
        NULL,                   // Ptr to OutBuffer
        0,                      // Length of OutBuffer
        &junk,                  // BytesReturned
        0))                     // Ptr to Overlapped structure
    {
        DWORD error = GetLastError();
        SetLastMsg("DeviceIoControl failed 0x%lx\n", error);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        ret = FALSE;
    }
    else
    {
        ret = TRUE;
    }

    DeviceCloseHandle(hDevice);
    return ret;
}

BOOL MonitorModesUpdate(UINT index, UINT modeCount, PMonitorMode modes)
{
    SetLastMsg("Sucess");

    HANDLE hDevice = DeviceOpenHandle();
    if (hDevice == INVALID_HANDLE_VALUE || hDevice == NULL)
    {
        return FALSE;
    }

    BOOL ret = FALSE;
    DWORD junk = 0;
    size_t buflen = sizeof(UINT) * 2 + modeCount * sizeof(MonitorMode);
    PCtlMonitorModes pMonitorModes = (PCtlMonitorModes)malloc(buflen);
    if (pMonitorModes == NULL)
    {
        SetLastMsg("CtlMonitorModes malloc failed 0x%lx\n");
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        return FALSE;
    }

    pMonitorModes->ConnectorIndex = index;
    pMonitorModes->ModeCount = modeCount;
    for (UINT i = 0; i < modeCount; ++i)
    {
        pMonitorModes->Modes[i].Width = modes[i].width;
        pMonitorModes->Modes[i].Height = modes[i].height;
        pMonitorModes->Modes[i].Sync = modes[i].sync;
    }
    if (!DeviceIoControl(
        hDevice,
        IOCTL_CHANGER_IDD_UPDATE_MONITOR_MODE,
        pMonitorModes,               // Ptr to InBuffer
        buflen,                     // Length of InBuffer
        NULL,                       // Ptr to OutBuffer
        0,                          // Length of OutBuffer
        &junk,                      // BytesReturned
        0))                         // Ptr to Overlapped structure
    {
        DWORD error = GetLastError();
        SetLastMsg("DeviceIoControl failed 0x%lx\n", error);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        ret = FALSE;
    }
    else
    {
        ret = TRUE;
    }

    free(pMonitorModes);
    DeviceCloseHandle(hDevice);
    return ret;
}

VOID WINAPI
CreationCallback(
    _In_ HSWDEVICE hSwDevice,
    _In_ HRESULT hrCreateResult,
    _In_opt_ PVOID pContext,
    _In_opt_ PCWSTR pszDeviceInstanceId
)
{
    HANDLE hEvent = *(HANDLE*)pContext;

    SetEvent(hEvent);
    UNREFERENCED_PARAMETER(hSwDevice);
    UNREFERENCED_PARAMETER(hrCreateResult);
    // printf("Idd device %ls created\n", pszDeviceInstanceId);
}

BOOLEAN
GetDevicePath(
    _In_ LPCGUID InterfaceGuid,
    _Out_writes_(BufLen) PTCHAR DevicePath,
    _In_ size_t BufLen
)
{
    CONFIGRET cr = CR_SUCCESS;
    PTSTR deviceInterfaceList = NULL;
    ULONG deviceInterfaceListLength = 0;
    PTSTR nextInterface;
    HRESULT hr = E_FAIL;
    BOOLEAN bRet = TRUE;

    cr = CM_Get_Device_Interface_List_Size(
        &deviceInterfaceListLength,
        (LPGUID)InterfaceGuid,
        NULL,
        CM_GET_DEVICE_INTERFACE_LIST_ALL_DEVICES);
    if (cr != CR_SUCCESS)
    {
        SetLastMsg("Error GetDevicePath 0x%x retrieving device interface list size.\n", cr);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }

        goto clean0;
    }

    // CAUTION: BUG here. deviceInterfaceListLength is greater than 1, even device was not created...
    if (deviceInterfaceListLength <= 1)
    {
        SetLastMsg("Error: GetDevicePath No active device interfaces found. Is the sample driver loaded?\n");
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        bRet = FALSE;
        goto clean0;
    }

    deviceInterfaceList = (PTSTR)malloc(deviceInterfaceListLength * sizeof(TCHAR));
    if (deviceInterfaceList == NULL)
    {
        SetLastMsg("Error GetDevicePath allocating memory for device interface list.\n");
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        bRet = FALSE;
        goto clean0;
    }
    ZeroMemory(deviceInterfaceList, deviceInterfaceListLength * sizeof(TCHAR));

    for (int i = 0; i < 3 && _tcslen(deviceInterfaceList) == 0; i++)
    {
        // CAUTION: BUG here. deviceInterfaceList is NULL, even device was not created...
        cr = CM_Get_Device_Interface_List(
            (LPGUID)InterfaceGuid,
            NULL,
            deviceInterfaceList,
            deviceInterfaceListLength,
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT);
        if (cr != CR_SUCCESS)
        {
            SetLastMsg("Error GetDevicePath 0x%x retrieving device interface list.\n", cr);
            if (g_printMsg)
            {
                printf(g_lastMsg);
            }
            goto clean0;
        }
        _tprintf(_T("get deviceInterfaceList %s\n"), deviceInterfaceList);
        Sleep(1000);
    }

    nextInterface = deviceInterfaceList + _tcslen(deviceInterfaceList) + 1;
#ifdef UNICODE
    if (*nextInterface != UNICODE_NULL) {
#else
    if (*nextInterface != ANSI_NULL) {
#endif
        printf("Warning: More than one device interface instance found. \n"
            "Selecting first matching device.\n\n");
    }

    printf("begin copy device path\n");
    hr = StringCchCopy(DevicePath, BufLen, deviceInterfaceList);
    if (FAILED(hr))
    {
        SetLastMsg("Error: GetDevicePath StringCchCopy failed with HRESULT 0x%x", hr);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        bRet = FALSE;
        goto clean0;
    }

clean0:
    if (deviceInterfaceList != NULL)
    {
        free(deviceInterfaceList);
    }
    if (CR_SUCCESS != cr)
    {
        bRet = FALSE;
    }

    return bRet;
}

BOOLEAN GetDevicePath2(
    _In_ LPCGUID InterfaceGuid,
    _Out_writes_(BufLen) PTCHAR DevicePath,
    _In_ size_t BufLen
)
{
    HANDLE                              hDevice = INVALID_HANDLE_VALUE;
    PSP_DEVICE_INTERFACE_DETAIL_DATA    deviceInterfaceDetailData = NULL;
    ULONG                               predictedLength = 0;
    ULONG                               requiredLength = 0;
    ULONG                               bytes;
    HDEVINFO                            hardwareDeviceInfo;
    SP_DEVICE_INTERFACE_DATA            deviceInterfaceData;
    BOOLEAN                             status = FALSE;
    HRESULT                             hr;

    hardwareDeviceInfo = SetupDiGetClassDevs(
        InterfaceGuid,
        NULL, // Define no enumerator (global)
        NULL, // Define no
        (DIGCF_PRESENT | // Only Devices present
            DIGCF_DEVICEINTERFACE)); // Function class devices.
    if (INVALID_HANDLE_VALUE == hardwareDeviceInfo)
    {
        SetLastMsg("Idd device: SetupDiGetClassDevs failed, last error 0x%x\n", GetLastError());
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        return FALSE;
    }

    deviceInterfaceData.cbSize = sizeof(SP_DEVICE_INTERFACE_DATA);

    if (!SetupDiEnumDeviceInterfaces(hardwareDeviceInfo,
        0, // No care about specific PDOs
        InterfaceGuid,
        0, //
        &deviceInterfaceData))
    {
        SetLastMsg("Idd device: SetupDiEnumDeviceInterfaces failed, last error 0x%x\n", GetLastError());
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        goto Clean0;
    }

    //
    // Allocate a function class device data structure to receive the
    // information about this particular device.
    //
    SetupDiGetDeviceInterfaceDetail(
        hardwareDeviceInfo,
        &deviceInterfaceData,
        NULL, // probing so no output buffer yet
        0, // probing so output buffer length of zero
        &requiredLength,
        NULL);//not interested in the specific dev-node

    if (ERROR_INSUFFICIENT_BUFFER != GetLastError())
    {
        SetLastMsg("Idd device: SetupDiGetDeviceInterfaceDetail failed, last error 0x%x\n", GetLastError());
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        goto Clean0;
    }

    predictedLength = requiredLength;
    deviceInterfaceDetailData = (PSP_DEVICE_INTERFACE_DETAIL_DATA)HeapAlloc(
        GetProcessHeap(),
        HEAP_ZERO_MEMORY,
        predictedLength
    );

    if (deviceInterfaceDetailData)
    {
        deviceInterfaceDetailData->cbSize =
            sizeof(SP_DEVICE_INTERFACE_DETAIL_DATA);
    }
    else
    {
        SetLastMsg("Idd device: HeapAlloc failed, last error 0x%x\n", GetLastError());
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        goto Clean0;
    }

    if (!SetupDiGetDeviceInterfaceDetail(
        hardwareDeviceInfo,
        &deviceInterfaceData,
        deviceInterfaceDetailData,
        predictedLength,
        &requiredLength,
        NULL))
    {
        SetLastMsg("Idd device: SetupDiGetDeviceInterfaceDetail failed, last error 0x%x\n", GetLastError());
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        goto Clean1;
    }

    hr = StringCchCopy(DevicePath, BufLen, deviceInterfaceDetailData->DevicePath);
    if (FAILED(hr))
    {
        SetLastMsg("Error: StringCchCopy failed with HRESULT 0x%x", hr);
        if (g_printMsg)
        {
            printf(g_lastMsg);
        }
        status = FALSE;
        goto Clean1;
    }
    else
    {
        status = TRUE;
    }

Clean1:
    (VOID)HeapFree(GetProcessHeap(), 0, deviceInterfaceDetailData);
Clean0:
    (VOID)SetupDiDestroyDeviceInfoList(hardwareDeviceInfo);
    return status;
}

// https://stackoverflow.com/questions/67164846/createfile-fails-unless-i-disable-enable-my-device
HANDLE DeviceOpenHandle()
{
    SetLastMsg("Sucess");

    // const int maxDevPathLen = 256;
    TCHAR devicePath[256] = { 0 };
    HANDLE hDevice = INVALID_HANDLE_VALUE;
    do
    {
        if (FALSE == GetDevicePath2(
            &GUID_DEVINTERFACE_IDD_DRIVER_DEVICE,
            devicePath,
            sizeof(devicePath) / sizeof(devicePath[0])))
        {
            break;
        }
        if (_tcslen(devicePath) == 0)
        {
            SetLastMsg("GetDevicePath got empty device path\n");
            if (g_printMsg)
            {
                printf(g_lastMsg);
            }
            break;
        }

        _tprintf(_T("Idd device: try open %s\n"), devicePath);
        hDevice = CreateFile(
            devicePath,
            GENERIC_READ | GENERIC_WRITE,
            // FILE_SHARE_READ | FILE_SHARE_WRITE,
            0,
            NULL, // no SECURITY_ATTRIBUTES structure
            OPEN_EXISTING, // No special create flags
            0, // No special attributes
            NULL
        );
        if (hDevice == INVALID_HANDLE_VALUE || hDevice == NULL)
        {
            DWORD error = GetLastError();
            SetLastMsg("CreateFile failed 0x%lx\n", error);
            if (g_printMsg)
            {
                printf(g_lastMsg);
            }
        }
    } while (0);

    return hDevice;
}

VOID DeviceCloseHandle(HANDLE handle)
{
    if (handle != INVALID_HANDLE_VALUE && handle != NULL)
    {
        CloseHandle(handle);
    }
}

VOID SetPrintErrMsg(BOOL b)
{
    g_printMsg = (b == TRUE);
}
