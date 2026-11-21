#include "pch.h"
#include "Common.h"
#include "PrinterServiceState.h"
#include <winspool.h>
#include <memutil.h>
#include <procutil.h>
#include <strutil.h>
#include <cstddef>
#include <memory>
#include <string>
#include <vector>

#pragma comment(lib, "Winspool.lib")
#pragma comment(lib, "Advapi32.lib")

namespace {
constexpr DWORD kPrinterInfoLevel = 2;
constexpr DWORD kPrinterDefaultsLevel = 8;
constexpr wchar_t kRestoreAction[] = L"RestoreUpgradePrinter";
constexpr size_t kPointers[] = {
    offsetof(PRINTER_INFO_2W, pServerName), offsetof(PRINTER_INFO_2W, pPrinterName),
    offsetof(PRINTER_INFO_2W, pShareName), offsetof(PRINTER_INFO_2W, pPortName),
    offsetof(PRINTER_INFO_2W, pDriverName), offsetof(PRINTER_INFO_2W, pComment),
    offsetof(PRINTER_INFO_2W, pLocation), offsetof(PRINTER_INFO_2W, pDevMode),
    offsetof(PRINTER_INFO_2W, pSepFile), offsetof(PRINTER_INFO_2W, pPrintProcessor),
    offsetof(PRINTER_INFO_2W, pDatatype), offsetof(PRINTER_INFO_2W, pParameters),
    offsetof(PRINTER_INFO_2W, pSecurityDescriptor),
};

struct CaString {
    LPWSTR value = nullptr;
    ~CaString() { ReleaseStr(value); }
};

struct Printer {
    HANDLE value = nullptr;
    ~Printer() { if (value) ClosePrinter(value); }
};

struct CaBuffer {
    BYTE* value = nullptr;
    DWORD_PTR size = 0;
    ~CaBuffer() { ReleaseMem(value); }
};

bool Owned(const PRINTER_INFO_2W& queue, const std::wstring& app) {
    const std::wstring name = app + L" Printer";
    if (queue.pComment && std::wstring(L"RustDesk PDF printer v1:") + app == queue.pComment)
        return true;
    return queue.pPrinterName && queue.pDriverName && queue.pPortName
        && _wcsicmp(queue.pPrinterName, name.c_str()) == 0
        && _wcsicmp(queue.pPortName, name.c_str()) == 0
        && _wcsicmp(queue.pDriverName, L"RustDesk v4 Printer Driver") == 0;
}

HRESULT FindQueue(const std::wstring& app, std::wstring& name) {
    DWORD needed = 0, count = 0;
    if (!EnumPrintersW(PRINTER_ENUM_LOCAL, nullptr, kPrinterInfoLevel, nullptr, 0, &needed, &count)
        && GetLastError() != ERROR_INSUFFICIENT_BUFFER) return HRESULT_FROM_WIN32(GetLastError());
    if (!needed) return S_FALSE;
    std::vector<BYTE> data(needed);
    if (!EnumPrintersW(PRINTER_ENUM_LOCAL, nullptr, kPrinterInfoLevel, data.data(), needed, &needed, &count))
        return HRESULT_FROM_WIN32(GetLastError());
    if (count > data.size() / sizeof(PRINTER_INFO_2W)) return E_INVALIDARG;
    const auto queues = reinterpret_cast<const PRINTER_INFO_2W*>(data.data());
    for (DWORD index = 0; index < count; ++index) {
        if (!Owned(queues[index], app)) continue;
        if (!name.empty()) return HRESULT_FROM_WIN32(ERROR_DUP_NAME);
        name = queues[index].pPrinterName;
    }
    return name.empty() ? S_FALSE : S_OK;
}

HRESULT ReadQueue(const std::wstring& name, std::vector<BYTE>& data) {
    Printer printer;
    PRINTER_DEFAULTSW defaults = { nullptr, nullptr, PRINTER_READ };
    if (!OpenPrinterW(const_cast<LPWSTR>(name.c_str()), &printer.value, &defaults))
        return HRESULT_FROM_WIN32(GetLastError());
    DWORD needed = 0;
    if (GetPrinterW(printer.value, kPrinterInfoLevel, nullptr, 0, &needed)
        || GetLastError() != ERROR_INSUFFICIENT_BUFFER) return E_FAIL;
    DWORD defaultsSize = 0;
    if (GetPrinterW(printer.value, kPrinterDefaultsLevel, nullptr, 0, &defaultsSize)
        || GetLastError() != ERROR_INSUFFICIENT_BUFFER) return E_FAIL;
    constexpr size_t alignment = alignof(PRINTER_INFO_8W);
    const size_t offset = (static_cast<size_t>(needed) + alignment - 1) / alignment * alignment;
    data.resize(offset + defaultsSize);
    if (!GetPrinterW(printer.value, kPrinterInfoLevel, data.data(), needed, &needed))
        return HRESULT_FROM_WIN32(GetLastError());
    if (!GetPrinterW(printer.value, kPrinterDefaultsLevel, data.data() + offset, defaultsSize, &defaultsSize))
        return HRESULT_FROM_WIN32(GetLastError());
    auto global = reinterpret_cast<PRINTER_INFO_8W*>(data.data() + offset);
    reinterpret_cast<PRINTER_INFO_2W*>(data.data())->pDevMode = global->pDevMode;
    global->pDevMode = nullptr;
    return S_OK;
}

// GetPrinter keeps its strings, private DEVMODE and security descriptor in one buffer.
// Store offsets in MSI's rollback data, never process-local pointers or temporary files.
HRESULT Rebase(BYTE* data, size_t size, bool restore) {
    if (size < sizeof(PRINTER_INFO_2W)) return E_INVALIDARG;
    const auto base = reinterpret_cast<ULONG_PTR>(data);
    for (const auto field : kPointers) {
        ULONG_PTR pointer = 0;
        memcpy(&pointer, data + field, sizeof(pointer));
        if (!pointer) continue;
        const auto offset = restore ? pointer : pointer - base;
        if (offset < sizeof(PRINTER_INFO_2W) || offset >= size) return E_INVALIDARG;
        pointer = restore ? base + offset : offset;
        memcpy(data + field, &pointer, sizeof(pointer));
    }
    return S_OK;
}

HRESULT ProductProperty(LPCWSTR product, LPCWSTR property, std::wstring& value) {
    DWORD length = 0;
    UINT error = MsiGetProductInfoExW(product, nullptr, MSIINSTALLCONTEXT_MACHINE, property, nullptr, &length);
    if (error != ERROR_SUCCESS) return HRESULT_FROM_WIN32(error);
    value.resize(static_cast<size_t>(length) + 1);
    ++length;
    error = MsiGetProductInfoExW(product, nullptr, MSIINSTALLCONTEXT_MACHINE, property, &value[0], &length);
    if (error != ERROR_SUCCESS) return HRESULT_FROM_WIN32(error);
    value.resize(length);
    return value.empty() ? E_INVALIDARG : S_OK;
}

HRESULT OriginalExecutable(LPCWSTR product, const std::wstring& app, std::wstring& exe) {
    std::wstring directory;
    HRESULT result = ProductProperty(product, INSTALLPROPERTY_INSTALLLOCATION, directory);
    if (FAILED(result)) return result;
    if (app.find_first_of(L"\\/:\"") != std::wstring::npos) return E_INVALIDARG;
    if (directory.back() != L'\\') directory += L'\\';
    exe = directory + app + L".exe";
    return S_OK;
}

HRESULT InstallOriginalQueue(LPCWSTR product, const std::wstring& app) {
    std::wstring exe;
    HRESULT result = OriginalExecutable(product, app, exe);
    if (FAILED(result)) return result;
    HANDLE process = nullptr;
    result = ProcExec(exe.c_str(), L"--install-remote-printer", SW_HIDE, &process);
    if (FAILED(result)) return result;
    DWORD exitCode = 0;
    result = ProcWaitForCompletion(process, INFINITE, &exitCode);
    CloseHandle(process);
    if (SUCCEEDED(result) && exitCode != ERROR_SUCCESS) result = HRESULT_FROM_WIN32(exitCode);
    return result;
}

HRESULT RestoreOriginalService(LPCWSTR product, const std::wstring& app, const PrinterServiceState& original) {
    std::wstring exe;
    HRESULT result = OriginalExecutable(product, app, exe);
    if (FAILED(result)) return result;
    const auto binary = L"\"" + exe + L"\" --service";
    const auto display = app + L" Service";
    const bool running = original.current == SERVICE_RUNNING;
    // A running service can have future starts disabled; restore that setting after starting it.
    const DWORD startType = running ? SERVICE_DEMAND_START : original.startType;
    using Service = std::unique_ptr<SC_HANDLE__, decltype(&CloseServiceHandle)>;
    Service manager(OpenSCManagerW(nullptr, nullptr, SC_MANAGER_CREATE_SERVICE | SC_MANAGER_CONNECT), CloseServiceHandle);
    if (!manager) return HRESULT_FROM_WIN32(GetLastError());
    Service service(CreateServiceW(manager.get(), app.c_str(), display.c_str(), SERVICE_START | SERVICE_CHANGE_CONFIG,
        SERVICE_WIN32_OWN_PROCESS, startType, SERVICE_ERROR_NORMAL,
        binary.c_str(), nullptr, nullptr, nullptr, nullptr, nullptr), CloseServiceHandle);
    if (!service && GetLastError() == ERROR_SERVICE_EXISTS) {
        service.reset(OpenServiceW(manager.get(), app.c_str(), SERVICE_START | SERVICE_CHANGE_CONFIG));
        if (service && !ChangeServiceConfigW(service.get(), SERVICE_NO_CHANGE, startType,
            SERVICE_NO_CHANGE, binary.c_str(), nullptr, nullptr, nullptr, nullptr, nullptr, nullptr))
            return HRESULT_FROM_WIN32(GetLastError());
    }
    if (!service) return HRESULT_FROM_WIN32(GetLastError());
    if (running && !StartServiceW(service.get(), 0, nullptr) && GetLastError() != ERROR_SERVICE_ALREADY_RUNNING)
        result = HRESULT_FROM_WIN32(GetLastError());
    if (!ChangeServiceConfigW(service.get(), SERVICE_NO_CHANGE, original.startType,
        SERVICE_NO_CHANGE, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr, nullptr))
        return HRESULT_FROM_WIN32(GetLastError());
    return result;
}

HRESULT ApplyQueue(const std::wstring& name, const PRINTER_INFO_2W& info) {
    Printer printer;
    PRINTER_DEFAULTSW defaults = { nullptr, nullptr, PRINTER_ALL_ACCESS };
    if (!OpenPrinterW(const_cast<LPWSTR>(name.c_str()), &printer.value, &defaults))
        return HRESULT_FROM_WIN32(GetLastError());
    // Restore the original driver's complete settings, including its private DEVMODE.
    auto restored = info;
    std::vector<BYTE> current;
    if (info.pDriverName && _wcsicmp(info.pDriverName, L"Microsoft Print To PDF") == 0) {
        const HRESULT result = ReadQueue(name, current);
        if (FAILED(result)) return result;
        // A recreated PDF queue has a new protected pipe; the old port has been removed.
        restored.pPortName = reinterpret_cast<const PRINTER_INFO_2W*>(current.data())->pPortName;
    }
    restored.pDevMode = nullptr;
    if (!SetPrinterW(printer.value, kPrinterInfoLevel, reinterpret_cast<BYTE*>(&restored), 0))
        return HRESULT_FROM_WIN32(GetLastError());
    // Level 2 updates the caller's preferences; MSI runs as SYSTEM, so restore global defaults explicitly.
    PRINTER_INFO_8W global = { info.pDevMode };
    if (!SetPrinterW(printer.value, kPrinterDefaultsLevel, reinterpret_cast<BYTE*>(&global), 0))
        return HRESULT_FROM_WIN32(GetLastError());
    if ((info.Status & PRINTER_STATUS_PAUSED)
        && !SetPrinterW(printer.value, 0, nullptr, PRINTER_CONTROL_PAUSE))
        return HRESULT_FROM_WIN32(GetLastError());
    return S_OK;
}

HRESULT AppendQueueSnapshot(const std::wstring& name, LPWSTR* actionData) {
    std::vector<BYTE> bytes;
    HRESULT result = ReadQueue(name, bytes);
    if (FAILED(result)) return result;
    auto info = reinterpret_cast<const PRINTER_INFO_2W*>(bytes.data());
    SECURITY_DESCRIPTOR_CONTROL control = 0;
    DWORD revision = 0;
    if (!info->pSecurityDescriptor || !GetSecurityDescriptorControl(info->pSecurityDescriptor, &control, &revision)
        || !(control & SE_SELF_RELATIVE)) return E_INVALIDARG;
    result = Rebase(bytes.data(), bytes.size(), false);
    if (FAILED(result)) return result;
    return WcaWriteStreamToCaData(bytes.data(), bytes.size(), actionData);
}

HRESULT ScheduleRecovery() {
    CaString product, actionData;
    HRESULT result = WcaGetProperty(L"WIX_UPGRADE_DETECTED", &product.value);
    if (FAILED(result)) return result;
    if (!product.value || !*product.value) return S_OK;
    std::wstring appName, name;
    result = ProductProperty(product.value, INSTALLPROPERTY_INSTALLEDPRODUCTNAME, appName);
    if (SUCCEEDED(result)) result = FindQueue(appName, name);
    if (FAILED(result)) return result;
    PrinterServiceState service;
    result = ReadPrinterServiceState(appName.c_str(), service);
    if (FAILED(result)) return result;
    if (name.empty() && service.current == 0) return S_OK;
    result = WcaWriteStringToCaData(product.value, &actionData.value);
    if (SUCCEEDED(result)) result = WcaWriteIntegerToCaData(service.current, &actionData.value);
    if (SUCCEEDED(result)) result = WcaWriteIntegerToCaData(service.startType, &actionData.value);
    if (SUCCEEDED(result) && !name.empty()) result = AppendQueueSnapshot(name, &actionData.value);
    if (FAILED(result)) return result;
    WcaLog(LOGMSG_STANDARD, "Scheduling upgrade recovery for %ls", appName.c_str());
    result = WcaSetProperty(kRestoreAction, actionData.value);
    if (SUCCEEDED(result) && service.current != 0)
        result = WcaSetProperty(L"StopUpgradePrinterService", appName.c_str());
    return result;
}

HRESULT RestoreRecovery() {
    CaString actionData, product;
    HRESULT result = WcaGetProperty(L"CustomActionData", &actionData.value);
    if (FAILED(result)) return result;
    LPWSTR cursor = actionData.value;
    result = WcaReadStringFromCaData(&cursor, &product.value);
    PrinterServiceState original;
    if (SUCCEEDED(result)) result = WcaReadIntegerFromCaData(&cursor, &original.current);
    if (SUCCEEDED(result)) result = WcaReadIntegerFromCaData(&cursor, &original.startType);
    std::wstring app, name;
    if (SUCCEEDED(result)) result = ProductProperty(product.value, INSTALLPROPERTY_INSTALLEDPRODUCTNAME, app);
    if (FAILED(result)) return result;
    // With no old queue, the service state is the last field in the rollback data.
    if (!cursor) return original.current ? RestoreOriginalService(product.value, app, original) : E_INVALIDARG;
    CaBuffer data;
    result = WcaReadStreamFromCaData(&cursor, &data.value, &data.size);
    if (SUCCEEDED(result)) result = Rebase(data.value, data.size, true);
    if (FAILED(result)) return result;
    result = FindQueue(app, name);
    if (result == S_FALSE) {
        result = InstallOriginalQueue(product.value, app);
        if (SUCCEEDED(result)) result = FindQueue(app, name);
        if (result == S_FALSE) result = HRESULT_FROM_WIN32(ERROR_PRINTER_NOT_FOUND);
    }
    if (SUCCEEDED(result)) result = ApplyQueue(name, *reinterpret_cast<PRINTER_INFO_2W*>(data.value));
    if (original.current != 0) {
        const HRESULT service = RestoreOriginalService(product.value, app, original);
        if (FAILED(service)) WcaLog(LOGMSG_STANDARD, "Restoring the original service failed: 0x%08lx", service);
        if (SUCCEEDED(result)) result = service;
    }
    if (SUCCEEDED(result)) WcaLog(LOGMSG_STANDARD, "Restored upgrade printer %ls", name.c_str());
    return result;
}

UINT RunAction(MSIHANDLE installer, bool restore) {
    HRESULT result = WcaInitialize(installer, restore ? "RestoreUpgradePrinter" : "ScheduleUpgradePrinterRecovery");
    if (FAILED(result)) return ERROR_INSTALL_FAILURE;
    try {
        result = restore ? RestoreRecovery() : ScheduleRecovery();
    } catch (const std::exception&) {
        result = E_OUTOFMEMORY;
    }
    if (FAILED(result)) WcaLog(LOGMSG_STANDARD, "Upgrade printer recovery failed: 0x%08lx", result);
    return WcaFinalize(SUCCEEDED(result) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE);
}
}

extern "C" UINT __stdcall ScheduleUpgradePrinterRecovery(MSIHANDLE installer) {
    return RunAction(installer, false);
}

extern "C" UINT __stdcall RestoreUpgradePrinter(MSIHANDLE installer) {
    return RunAction(installer, true);
}
