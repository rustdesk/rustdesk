#include "pch.h"
#include <winspool.h>
#include <strutil.h>
#include <string>
#include <vector>

#pragma comment(lib, "Winspool.lib")

namespace {
constexpr DWORD kQueueInfoLevel = 2;
constexpr wchar_t kOwnerPrefix[] = L"RustDesk PDF printer v1:";
constexpr wchar_t kLegacyDriver[] = L"RustDesk v4 Printer Driver";

HRESULT IsPrinterPresent(const std::wstring& appName, bool& present) {
    present = false;
    DWORD needed = 0;
    DWORD count = 0;
    if (!EnumPrintersW(PRINTER_ENUM_LOCAL, nullptr, kQueueInfoLevel, nullptr, 0, &needed, &count)) {
        const DWORD error = GetLastError();
        if (error != ERROR_INSUFFICIENT_BUFFER) return HRESULT_FROM_WIN32(error);
    }
    if (needed == 0) return S_OK;
    std::vector<BYTE> buffer(needed);
    if (!EnumPrintersW(PRINTER_ENUM_LOCAL, nullptr, kQueueInfoLevel, buffer.data(), needed, &needed, &count))
        return HRESULT_FROM_WIN32(GetLastError());
    if (count > buffer.size() / sizeof(PRINTER_INFO_2W)) return HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
    const auto queues = reinterpret_cast<const PRINTER_INFO_2W*>(buffer.data());
    const std::wstring marker = std::wstring(kOwnerPrefix) + appName;
    const std::wstring name = appName + L" Printer";
    for (DWORD index = 0; index < count; ++index) {
        const auto& queue = queues[index];
        if (queue.pComment && marker == queue.pComment) present = true;
        if (queue.pPrinterName && queue.pDriverName && queue.pPortName
            && _wcsicmp(queue.pPrinterName, name.c_str()) == 0
            && _wcsicmp(queue.pDriverName, kLegacyDriver) == 0
            && _wcsicmp(queue.pPortName, name.c_str()) == 0) present = true;
    }
    return S_OK;
}

HRESULT ReadPrinterPresence(bool& present) {
    LPWSTR appName = nullptr;
    HRESULT result = WcaGetProperty(L"ProductName", &appName);
    if (SUCCEEDED(result) && (!appName || !*appName)) result = E_INVALIDARG;
    if (SUCCEEDED(result)) {
        try {
            result = IsPrinterPresent(appName, present);
        } catch (const std::exception&) {
            result = E_OUTOFMEMORY;
        }
    }
    if (appName) StrFree(appName);
    return result;
}
}

extern "C" UINT __stdcall CheckPdfPrinter(MSIHANDLE installer) {
    HRESULT result = WcaInitialize(installer, "CheckPdfPrinter");
    if (FAILED(result)) return ERROR_INSTALL_FAILURE;
    bool present = false;
    result = ReadPrinterPresence(present);
    if (SUCCEEDED(result)) {
        result = WcaSetProperty(L"PDFPRINTER_PRESENT", present ? L"1" : L"0");
    } else if (MsiEvaluateConditionW(installer, L"NOT WIX_UPGRADE_DETECTED") == MSICONDITION_TRUE) {
        WcaLog(LOGMSG_STANDARD, "Skipping optional PDF printer setup because queue detection failed (0x%08lx)", result);
        result = WcaSetProperty(L"PDFPRINTER_PRESENT", L"");
    }
    if (FAILED(result)) WcaLog(LOGMSG_STANDARD, "PDF printer detection failed: 0x%08lx", result);
    return WcaFinalize(SUCCEEDED(result) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE);
}

extern "C" UINT __stdcall SetPrinterInstallDefault(MSIHANDLE installer) {
    HRESULT result = WcaInitialize(installer, "SetPrinterInstallDefault");
    if (FAILED(result)) return ERROR_INSTALL_FAILURE;
    bool present = false;
    result = ReadPrinterPresence(present);
    if (FAILED(result))
        WcaLog(LOGMSG_STANDARD, "Printer detection failed: 0x%08lx; leaving installation disabled by default", result);
    result = WcaSetProperty(L"INSTALLPRINTER", SUCCEEDED(result) && present ? L"1" : L"0");
    return WcaFinalize(SUCCEEDED(result) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE);
}
