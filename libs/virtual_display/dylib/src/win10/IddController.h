#pragma once
#include <Windows.h>
#include <tchar.h>
#include <swdevice.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Install or Update RustDeskIddDriver.
 *
 * @param fullInfPath [in] Full path of the driver inf file.
 * @param rebootRequired [out] Indicates whether a restart is required.
 *
 * @return TRUE/FALSE. If FALSE returned, error message can be retrieved by GetLastMsg()
 *
 * @see GetLastMsg#GetLastMsg
 */
BOOL InstallUpdate(LPCTSTR fullInfPath, PBOOL rebootRequired);

/**
 * @brief Uninstall RustDeskIddDriver.
 *
 * @param fullInfPath [in] Full path of the driver inf file.
 * @param rebootRequired [out] Indicates whether a restart is required.
 *
 * @return TRUE/FALSE. If FALSE returned, error message can be retrieved by GetLastMsg()
 *
 * @see GetLastMsg#GetLastMsg
 */
BOOL Uninstall(LPCTSTR fullInfPath, PBOOL rebootRequired);

/**
 * @brief Check if RustDeskIddDriver device is created before.
 *        The driver device(adapter) should be single instance.
 *
 * @param created [out] Indicates whether the device is created before.
 *
 * @return TRUE/FALSE. If FALSE returned, error message can be retrieved by GetLastMsg()
 *
 * @see GetLastMsg#GetLastMsg
 *
 */
BOOL IsDeviceCreated(PBOOL created);

/**
 * @brief Create device.
 *        Only one device should be created.
 *        If device is installed earlier, this function returns FALSE.
 *
 * @param hSwDevice [out] Handler of software device, used by DeviceCreate(). Should be **NULL**.
 *
 * @return TRUE/FALSE. If FALSE returned, error message can be retrieved by GetLastMsg()
 *
 * @see GetLastMsg#GetLastMsg
 *
 */
BOOL DeviceCreate(PHSWDEVICE hSwDevice);

/**
 * @brief Close device.
 *
 * @param hSwDevice Handler of software device, used by SwDeviceClose().
 *
 */
VOID DeviceClose(HSWDEVICE hSwDevice);

/**
 * @brief Plug in monitor.
 *
 * @param index [in] Monitor index, should be 0, 1, 2.
 * @param edid  [in] Monitor edid.
 *                   0 Modified EDID from Dell S2719DGF
 *                   1 Modified EDID from Lenovo Y27fA
 * @param retries [in] Retry times. Retry 1 time / sec. 25~30 seconds may be good choices.
 *                     -1 is invalid.
 *                     0 means doing once and no retries.
 *                     1 means doing once and retry one time...
 *
 * @return TRUE/FALSE. If FALSE returned, error message can be retrieved by GetLastMsg()
 *
 * @see GetLastMsg#GetLastMsg
 *
 * @remark  Plug in monitor may fail if device is created in a very short time.
 *          System need some time to prepare the device.
 *
 */
BOOL MonitorPlugIn(UINT index, UINT edid, INT retries);

/**
 * @brief Plug out monitor.
 *
 * @param index [in] Monitor index, should be 0, 1, 2.
 *
 * @return TRUE/FALSE. If FALSE returned, error message can be retrieved by GetLastMsg()
 *
 * @see GetLastMsg#GetLastMsg
 *
 */
BOOL MonitorPlugOut(UINT index);

typedef struct _MonitorMode {
    DWORD width;
    DWORD height;
    // Sync affects frequency.
    DWORD sync;
} MonitorMode, *PMonitorMode;

/**
 * @brief Update monitor mode.
 *
 * @param index       [in] Monitor index, should be 0, 1, 2.
 * @param modeCount   [in] Monitor mode count.
 * @param MonitorMode [in] Monitor mode data.
 *
 * @return TRUE/FALSE. If FALSE returned, error message can be retrieved by GetLastMsg()
 *
 * @see GetLastMsg#GetLastMsg
 *
 */
BOOL MonitorModesUpdate(UINT index, UINT modeCount, PMonitorMode modes);

/**
 * @brief Get last error message.
 *
 * @return Message string. The string is at most 1024 bytes.
 *
 */
const char* GetLastMsg();

/**
 * @brief Set if print error message when debug.
 *
 * @param b [in] TRUE to enable printing message.
 *
 * @remark For now, no need to read environment variable to check if should print.
 *
 */
VOID SetPrintErrMsg(BOOL b);

#ifdef __cplusplus
}
#endif
