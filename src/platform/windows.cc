#include <windows.h>
#include <wtsapi32.h>
#include <tlhelp32.h>
#include <comdef.h>
#include <xpsprint.h>
#include <cstdio>
#include <cstdint>
#include <intrin.h>
#include <string>
#include <memory>
#include <shlobj.h> // NOLINT(build/include_order)
#include <userenv.h>
#include <versionhelpers.h>
#include <vector>
#include <sddl.h>
#include <memory>

extern "C" uint32_t get_session_user_info(PWSTR bufin, uint32_t nin, uint32_t id);

void flog(char const *fmt, ...)
{
    FILE *h = fopen("C:\\Windows\\temp\\test_rustdesk.log", "at");
    if (!h)
        return;
    va_list arg;
    va_start(arg, fmt);
    vfprintf(h, fmt, arg);
    va_end(arg);
    fclose(h);
}

static BOOL GetProcessUserName(DWORD processID, LPWSTR outUserName, DWORD inUserNameSize)
{
    BOOL ret = FALSE;
    HANDLE hProcess = NULL;
    HANDLE hToken = NULL;
    PTOKEN_USER tokenUser = NULL;
    wchar_t *userName = NULL;
    wchar_t *domainName = NULL;

    hProcess = OpenProcess(PROCESS_QUERY_INFORMATION, FALSE, processID);
    if (hProcess == NULL)
    {
        goto cleanup;
    }
    if (!OpenProcessToken(hProcess, TOKEN_QUERY, &hToken))
    {
        goto cleanup;
    }
    DWORD tokenInfoLength = 0;
    GetTokenInformation(hToken, TokenUser, NULL, 0, &tokenInfoLength);
    if (tokenInfoLength == 0)
    {
        goto cleanup;
    }
    tokenUser = (PTOKEN_USER)malloc(tokenInfoLength);
    if (tokenUser == NULL)
    {
        goto cleanup;
    }
    if (!GetTokenInformation(hToken, TokenUser, tokenUser, tokenInfoLength, &tokenInfoLength))
    {
        goto cleanup;
    }
    DWORD userSize = 0;
    DWORD domainSize = 0;
    SID_NAME_USE snu;
    LookupAccountSidW(NULL, tokenUser->User.Sid, NULL, &userSize, NULL, &domainSize, &snu);
    if (userSize == 0 || domainSize == 0)
    {
        goto cleanup;
    }
    userName = (wchar_t *)malloc((userSize + 1) * sizeof(wchar_t));
    if (userName == NULL)
    {
        goto cleanup;
    }
    domainName = (wchar_t *)malloc((domainSize + 1) * sizeof(wchar_t));
    if (domainName == NULL)
    {
        goto cleanup;
    }
    if (!LookupAccountSidW(NULL, tokenUser->User.Sid, userName, &userSize, domainName, &domainSize, &snu))
    {
        goto cleanup;
    }
    userName[userSize] = L'\0';
    domainName[domainSize] = L'\0';
    if (inUserNameSize <= userSize)
    {
        goto cleanup;
    }
    wcscpy(outUserName, userName);

    ret = TRUE;
cleanup:
    if (userName)
    {
        free(userName);
    }
    if (domainName)
    {
        free(domainName);
    }
    if (tokenUser != NULL)
    {
        free(tokenUser);
    }
    if (hToken != NULL)
    {
        CloseHandle(hToken);
    }
    if (hProcess != NULL)
    {
        CloseHandle(hProcess);
    }

    return ret;
}

// ultravnc has rdp support
// https://github.com/veyon/ultravnc/blob/master/winvnc/winvnc/service.cpp
// https://github.com/TigerVNC/tigervnc/blob/master/win/winvnc/VNCServerService.cxx
// https://blog.csdn.net/MA540213/article/details/84638264

DWORD GetLogonPid(DWORD dwSessionId, BOOL as_user)
{
    DWORD dwLogonPid = 0;
    HANDLE hSnap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (hSnap != INVALID_HANDLE_VALUE)
    {
        PROCESSENTRY32W procEntry;
        procEntry.dwSize = sizeof procEntry;

        if (Process32FirstW(hSnap, &procEntry))
            do
            {
                DWORD dwLogonSessionId = 0;
                if (_wcsicmp(procEntry.szExeFile, as_user ? L"explorer.exe" : L"winlogon.exe") == 0 &&
                    ProcessIdToSessionId(procEntry.th32ProcessID, &dwLogonSessionId) &&
                    dwLogonSessionId == dwSessionId)
                {
                    dwLogonPid = procEntry.th32ProcessID;
                    break;
                }
            } while (Process32NextW(hSnap, &procEntry));
        CloseHandle(hSnap);
    }
    return dwLogonPid;
}

static DWORD GetFallbackUserPid(DWORD dwSessionId)
{
    DWORD dwFallbackPid = 0;
    const wchar_t* fallbackUserProcs[] = {L"sihost.exe"};
    const int maxUsernameLen = 256;
    wchar_t sessionUsername[maxUsernameLen + 1] = {0};
    wchar_t processUsername[maxUsernameLen + 1] = {0};

    if (get_session_user_info(sessionUsername, maxUsernameLen, dwSessionId) == 0)
    {
        return 0;
    }
    HANDLE hSnap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (hSnap != INVALID_HANDLE_VALUE)
    {
        PROCESSENTRY32W procEntry;
        procEntry.dwSize = sizeof procEntry;

        if (Process32FirstW(hSnap, &procEntry))
            do
            {
                for (int i = 0; i < sizeof(fallbackUserProcs) / sizeof(fallbackUserProcs[0]); i++)
                {
                    DWORD dwProcessSessionId = 0;
                    if (_wcsicmp(procEntry.szExeFile, fallbackUserProcs[i]) == 0 &&
                        ProcessIdToSessionId(procEntry.th32ProcessID, &dwProcessSessionId) &&
                        dwProcessSessionId == dwSessionId)
                    {
                        memset(processUsername, 0, sizeof(processUsername));
                        if (GetProcessUserName(procEntry.th32ProcessID, processUsername, maxUsernameLen)) {
                            if (_wcsicmp(sessionUsername, processUsername) == 0)
                            {
                                dwFallbackPid = procEntry.th32ProcessID;
                                break;
                            }                           
                        }
                    }
                }
                if (dwFallbackPid != 0)
                {
                    break;
                }
            } while (Process32NextW(hSnap, &procEntry));
        CloseHandle(hSnap);
    }
    return dwFallbackPid;
}

// START the app as system
extern "C"
{
    // if should try WTSQueryUserToken?
    // https://stackoverflow.com/questions/7285666/example-code-a-service-calls-createprocessasuser-i-want-the-process-to-run-in
    BOOL GetSessionUserTokenWin(OUT LPHANDLE lphUserToken, DWORD dwSessionId, BOOL as_user, DWORD *pDwTokenPid)
    {
        BOOL bResult = FALSE;
        DWORD Id = GetLogonPid(dwSessionId, as_user);
        if (Id == 0)
        {
            Id = GetFallbackUserPid(dwSessionId);
        }
        if (pDwTokenPid)
            *pDwTokenPid = Id;
        if (HANDLE hProcess = OpenProcess(PROCESS_ALL_ACCESS, FALSE, Id))
        {
            bResult = OpenProcessToken(hProcess, TOKEN_ALL_ACCESS, lphUserToken);
            CloseHandle(hProcess);
        }
        return bResult;
    }

    bool is_windows_server()
    {
        return IsWindowsServer();
    }

    bool is_windows_10_or_greater()
    {
        return IsWindows10OrGreater();
    }

    HANDLE LaunchProcessWin(LPCWSTR cmd, DWORD dwSessionId, BOOL as_user, BOOL show, DWORD *pDwTokenPid)
    {
        HANDLE hProcess = NULL;
        HANDLE hToken = NULL;
        if (GetSessionUserTokenWin(&hToken, dwSessionId, as_user, pDwTokenPid))
        {
            STARTUPINFOW si;
            ZeroMemory(&si, sizeof si);
            si.cb = sizeof si;
            si.dwFlags = STARTF_USESHOWWINDOW;
            if (show)
            {
                si.lpDesktop = (LPWSTR)L"winsta0\\default";
                si.wShowWindow = SW_SHOW;
            }
            wchar_t buf[MAX_PATH];
            wcscpy_s(buf, MAX_PATH, cmd);
            PROCESS_INFORMATION pi;
            LPVOID lpEnvironment = NULL;
            DWORD dwCreationFlags = DETACHED_PROCESS;
            if (as_user)
            {

                CreateEnvironmentBlock(&lpEnvironment, // Environment block
                                       hToken,         // New token
                                       TRUE);          // Inheritance
            }
            if (lpEnvironment)
            {
                dwCreationFlags |= CREATE_UNICODE_ENVIRONMENT;
            }
            if (CreateProcessAsUserW(hToken, NULL, buf, NULL, NULL, FALSE, dwCreationFlags, lpEnvironment, NULL, &si, &pi))
            {
                CloseHandle(pi.hThread);
                hProcess = pi.hProcess;
            }
            CloseHandle(hToken);
            if (lpEnvironment)
                DestroyEnvironmentBlock(lpEnvironment);
        }
        return hProcess;
    }

    // Switch the current thread to the specified desktop
    static bool
    switchToDesktop(HDESK desktop)
    {
        HDESK old_desktop = GetThreadDesktop(GetCurrentThreadId());
        if (!SetThreadDesktop(desktop))
        {
            return false;
        }
        if (!CloseDesktop(old_desktop))
        {
            //
        }
        return true;
    }

    // https://github.com/TigerVNC/tigervnc/blob/8c6c584377feba0e3b99eecb3ef33b28cee318cb/win/rfb_win32/Service.cxx

    // Determine whether the thread's current desktop is the input one
    BOOL
    inputDesktopSelected()
    {
        HDESK current = GetThreadDesktop(GetCurrentThreadId());
        HDESK input = OpenInputDesktop(0, FALSE,
                                       DESKTOP_CREATEMENU | DESKTOP_CREATEWINDOW |
                                           DESKTOP_ENUMERATE | DESKTOP_HOOKCONTROL |
                                           DESKTOP_WRITEOBJECTS | DESKTOP_READOBJECTS |
                                           DESKTOP_SWITCHDESKTOP | GENERIC_WRITE);
        if (!input)
        {
            return FALSE;
        }

        DWORD size;
        char currentname[256];
        char inputname[256];

        if (!GetUserObjectInformation(current, UOI_NAME, currentname, sizeof(currentname), &size))
        {
            CloseDesktop(input);
            return FALSE;
        }
        if (!GetUserObjectInformation(input, UOI_NAME, inputname, sizeof(inputname), &size))
        {
            CloseDesktop(input);
            return FALSE;
        }
        CloseDesktop(input);
        // flog("%s %s\n", currentname, inputname);
        return strcmp(currentname, inputname) == 0 ? TRUE : FALSE;
    }

    // Switch the current thread into the input desktop
    bool
    selectInputDesktop()
    {
        // - Open the input desktop
        HDESK desktop = OpenInputDesktop(0, FALSE,
                                         DESKTOP_CREATEMENU | DESKTOP_CREATEWINDOW |
                                             DESKTOP_ENUMERATE | DESKTOP_HOOKCONTROL |
                                             DESKTOP_WRITEOBJECTS | DESKTOP_READOBJECTS |
                                             DESKTOP_SWITCHDESKTOP | GENERIC_WRITE);
        if (!desktop)
        {
            return false;
        }

        // - Switch into it
        if (!switchToDesktop(desktop))
        {
            CloseDesktop(desktop);
            return false;
        }

        // ***
        DWORD size = 256;
        char currentname[256];
        if (GetUserObjectInformation(desktop, UOI_NAME, currentname, 256, &size))
        {
            //
        }

        return true;
    }

    int handleMask(uint8_t *rwbuffer, const uint8_t *mask, int width, int height, int bmWidthBytes, int bmHeight)
    {
        auto andMask = mask;
        auto andMaskSize = bmWidthBytes * bmHeight;
        auto offset = height * bmWidthBytes;
        auto xorMask = mask + offset;
        auto xorMaskSize = andMaskSize - offset;
        int doOutline = 0;
        for (int y = 0; y < height; y++)
        {
            for (int x = 0; x < width; x++)
            {
                int byte = y * bmWidthBytes + x / 8;
                int bit = 7 - x % 8;

                if (byte < andMaskSize && !(andMask[byte] & (1 << bit)))
                {
                    // Valid pixel, so make it opaque
                    rwbuffer[3] = 0xff;

                    // Black or white?
                    if (xorMask[byte] & (1 << bit))
                        rwbuffer[0] = rwbuffer[1] = rwbuffer[2] = 0xff;
                    else
                        rwbuffer[0] = rwbuffer[1] = rwbuffer[2] = 0;
                }
                else if (byte < xorMaskSize && xorMask[byte] & (1 << bit))
                {
                    // Replace any XORed pixels with black, because RFB doesn't support
                    // XORing of cursors.  XORing is used for the I-beam cursor, which is most
                    // often used over a white background, but also sometimes over a black
                    // background.  We set the XOR'd pixels to black, then draw a white outline
                    // around the whole cursor.

                    rwbuffer[0] = rwbuffer[1] = rwbuffer[2] = 0;
                    rwbuffer[3] = 0xff;

                    doOutline = 1;
                }
                else
                {
                    // Transparent pixel
                    rwbuffer[0] = rwbuffer[1] = rwbuffer[2] = rwbuffer[3] = 0;
                }

                rwbuffer += 4;
            }
        }
        return doOutline;
    }

    void drawOutline(uint8_t *out0, const uint8_t *in0, int width, int height, int out0_size)
    {
        auto in = in0;
        auto out0_end = out0 + out0_size;
        auto offset = width * 4 + 4;
        auto out = out0 + offset;
        for (int y = 0; y < height; y++)
        {
            for (int x = 0; x < width; x++)
            {
                // Visible pixel?
                if (in[3] > 0)
                {
                    auto n = 4 * 3;
                    auto p = out - (width + 2) * 4 - 4;
                    // Outline above...
                    if (p >= out0 && p + n <= out0_end)
                        memset(p, 0xff, n);
                    // ...besides...
                    p = out - 4;
                    if (p + n <= out0_end)
                        memset(p, 0xff, n);
                    // ...and above
                    p = out + (width + 2) * 4 - 4;
                    if (p + n <= out0_end)
                        memset(p, 0xff, n);
                }
                in += 4;
                out += 4;
            }
            // outline is slightly larger
            out += 2 * 4;
        }

        // Pass 2, overwrite with actual cursor
        in = in0;
        out = out0 + offset;
        for (int y = 0; y < height; y++)
        {
            for (int x = 0; x < width; x++)
            {
                if (in[3] > 0 && out + 4 <= out0_end)
                    memcpy(out, in, 4);
                in += 4;
                out += 4;
            }
            out += 2 * 4;
        }
    }

    int ffi(unsigned v)
    {
        static const int MultiplyDeBruijnBitPosition[32] =
            {
                0, 1, 28, 2, 29, 14, 24, 3, 30, 22, 20, 15, 25, 17, 4, 8,
                31, 27, 13, 23, 21, 19, 16, 7, 26, 12, 18, 6, 11, 5, 10, 9};
        return MultiplyDeBruijnBitPosition[((uint32_t)((v & -v) * 0x077CB531U)) >> 27];
    }

    int get_di_bits(uint8_t *out, HDC dc, HBITMAP hbmColor, int width, int height)
    {
        BITMAPV5HEADER bi;
        memset(&bi, 0, sizeof(BITMAPV5HEADER));

        bi.bV5Size = sizeof(BITMAPV5HEADER);
        bi.bV5Width = width;
        bi.bV5Height = -height; // Negative for top-down
        bi.bV5Planes = 1;
        bi.bV5BitCount = 32;
        bi.bV5Compression = BI_BITFIELDS;
        bi.bV5RedMask = 0x000000FF;
        bi.bV5GreenMask = 0x0000FF00;
        bi.bV5BlueMask = 0x00FF0000;
        bi.bV5AlphaMask = 0xFF000000;

        if (!GetDIBits(dc, hbmColor, 0, height,
                       out, (LPBITMAPINFO)&bi, DIB_RGB_COLORS))
            return 1;

        // We may not get the RGBA order we want, so shuffle things around
        int ridx, gidx, bidx, aidx;

        ridx = ffi(bi.bV5RedMask) / 8;
        gidx = ffi(bi.bV5GreenMask) / 8;
        bidx = ffi(bi.bV5BlueMask) / 8;
        // Usually not set properly
        aidx = 6 - ridx - gidx - bidx;

        if ((bi.bV5RedMask != ((unsigned)0xff << ridx * 8)) ||
            (bi.bV5GreenMask != ((unsigned)0xff << gidx * 8)) ||
            (bi.bV5BlueMask != ((unsigned)0xff << bidx * 8)))
            return 1;

        auto rwbuffer = out;
        for (int y = 0; y < height; y++)
        {
            for (int x = 0; x < width; x++)
            {
                uint8_t r, g, b, a;

                r = rwbuffer[ridx];
                g = rwbuffer[gidx];
                b = rwbuffer[bidx];
                a = rwbuffer[aidx];

                rwbuffer[0] = r;
                rwbuffer[1] = g;
                rwbuffer[2] = b;
                rwbuffer[3] = a;

                rwbuffer += 4;
            }
        }
        return 0;
    }

    void blank_screen(BOOL set)
    {
        if (set)
        {
            SendMessage(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER, (LPARAM)2);
        }
        else
        {
            SendMessage(HWND_BROADCAST, WM_SYSCOMMAND, SC_MONITORPOWER, (LPARAM)-1);
        }
    }

    void AddRecentDocument(PCWSTR path)
    {
        SHAddToRecentDocs(SHARD_PATHW, path);
    }

    DWORD get_current_session(BOOL include_rdp)
    {
        auto rdp_or_console = WTSGetActiveConsoleSessionId();
        if (!include_rdp)
            return rdp_or_console;
        PWTS_SESSION_INFOA pInfos;
        DWORD count;
        auto rdp = "rdp";
        auto nrdp = strlen(rdp);
        // https://github.com/rustdesk/rustdesk/discussions/937#discussioncomment-12373814 citrix session
        auto ica = "ica";
        auto nica = strlen(ica);
        if (WTSEnumerateSessionsA(WTS_CURRENT_SERVER_HANDLE, NULL, 1, &pInfos, &count))
        {
            for (DWORD i = 0; i < count; i++)
            {
                auto info = pInfos[i];
                if (info.State == WTSActive)
                {
                    if (info.pWinStationName == NULL)
                        continue;
                    if (!stricmp(info.pWinStationName, "console"))
                    {
                        auto id = info.SessionId;
                        WTSFreeMemory(pInfos);
                        return id;
                    }
                    if (!strnicmp(info.pWinStationName, rdp, nrdp) || !strnicmp(info.pWinStationName, ica, nica))
                    {
                        rdp_or_console = info.SessionId;
                    }
                }
            }
            WTSFreeMemory(pInfos);
        }
        return rdp_or_console;
    }

    uint32_t get_active_user(PWSTR bufin, uint32_t nin, BOOL rdp)
    {
        uint32_t nout = 0;
        auto id = get_current_session(rdp);
        PWSTR buf = NULL;
        DWORD n = 0;
        if (WTSQuerySessionInformationW(WTS_CURRENT_SERVER_HANDLE, id, WTSUserName, &buf, &n))
        {
            if (buf)
            {
                nout = min(nin, n);
                memcpy(bufin, buf, nout);
                WTSFreeMemory(buf);
            }
        }
        return nout;
    }

    uint32_t get_session_user_info(PWSTR bufin, uint32_t nin, uint32_t id)
    {
        uint32_t nout = 0;
        PWSTR buf = NULL;
        DWORD n = 0;
        if (WTSQuerySessionInformationW(WTS_CURRENT_SERVER_HANDLE, id, WTSUserName, &buf, &n))
        {
            if (buf)
            {
                nout = min(nin, n);
                memcpy(bufin, buf, nout);
                WTSFreeMemory(buf);
            }
        }
        return nout;
    }

    void get_available_session_ids(PWSTR buf, uint32_t bufSize, BOOL include_rdp) {
        std::vector<std::wstring> sessionIds;
        PWTS_SESSION_INFOA pInfos = NULL;
        DWORD count;

        if (WTSEnumerateSessionsA(WTS_CURRENT_SERVER_HANDLE, 0, 1, &pInfos, &count)) {
            for (DWORD i = 0; i < count; i++) {
                auto info = pInfos[i];
                auto rdp = "rdp";
                auto nrdp = strlen(rdp);
                auto ica = "ica";
                auto nica = strlen(ica);
                if (info.State == WTSActive) {
                    if (info.pWinStationName == NULL)
                        continue;
                    if (info.SessionId == 65536 || info.SessionId == 655)
                        continue;

                    if (!stricmp(info.pWinStationName, "console")){
                        sessionIds.push_back(std::wstring(L"Console:") + std::to_wstring(info.SessionId));
                    }
                    else if (include_rdp && !strnicmp(info.pWinStationName, rdp, nrdp)) {
                        sessionIds.push_back(std::wstring(L"RDP:") + std::to_wstring(info.SessionId));
                    }
                    else if (include_rdp && !strnicmp(info.pWinStationName, ica, nica)) {
                        sessionIds.push_back(std::wstring(L"ICA:") + std::to_wstring(info.SessionId));
                    }
                }
            }
            WTSFreeMemory(pInfos);
        }

        std::wstring tmpStr;
        for (size_t i = 0; i < sessionIds.size(); i++) {
            if (i > 0) {
                tmpStr += L",";
            }
            tmpStr += sessionIds[i];
        }

        if (buf && !tmpStr.empty() && tmpStr.size() < bufSize) {
            wcsncpy_s(buf, bufSize, tmpStr.c_str(), tmpStr.size());
        }
    }
} // end of extern "C"

// below copied from https://github.com/TigerVNC/tigervnc/blob/master/vncviewer/win32.c
extern "C"
{
    static HANDLE thread;
    static DWORD thread_id;

    static HHOOK hook = 0;
    static HWND target_wnd = 0;
    static HWND default_hook_wnd = 0;
    static bool win_down = false;
    static bool stop_system_key_propagate = false;

    bool is_win_down()
    {
        return win_down;
    }

#define ARRAY_SIZE(a) (sizeof(a) / sizeof(*a))

    static int is_system_hotkey(int vkCode, WPARAM wParam)
    {
        switch (vkCode)
        {
        case VK_LWIN:
        case VK_RWIN:
            win_down = wParam == WM_KEYDOWN;
        case VK_SNAPSHOT:
            return 1;
        case VK_TAB:
            if (GetAsyncKeyState(VK_MENU) & 0x8000)
                return 1;
        case VK_ESCAPE:
            if (GetAsyncKeyState(VK_MENU) & 0x8000)
                return 1;
            if (GetAsyncKeyState(VK_CONTROL) & 0x8000)
                return 1;
        }
        return 0;
    }

    static LRESULT CALLBACK keyboard_hook(int nCode, WPARAM wParam, LPARAM lParam)
    {
        if (nCode >= 0)
        {
            KBDLLHOOKSTRUCT *msgInfo = (KBDLLHOOKSTRUCT *)lParam;

            // Grabbing everything seems to mess up some keyboard state that
            // FLTK relies on, so just grab the keys that we normally cannot.
            if (stop_system_key_propagate && is_system_hotkey(msgInfo->vkCode, wParam))
            {
                PostMessage(target_wnd, wParam, msgInfo->vkCode,
                            (msgInfo->scanCode & 0xff) << 16 |
                                (msgInfo->flags & 0xff) << 24);
                return 1;
            }
        }

        return CallNextHookEx(hook, nCode, wParam, lParam);
    }

    static DWORD WINAPI keyboard_thread(LPVOID data)
    {
        MSG msg;

        target_wnd = (HWND)data;

        // Make sure a message queue is created
        PeekMessage(&msg, NULL, 0, 0, PM_NOREMOVE | PM_NOYIELD);

        hook = SetWindowsHookEx(WH_KEYBOARD_LL, keyboard_hook, GetModuleHandle(0), 0);
        // If something goes wrong then there is not much we can do.
        // Just sit around and wait for WM_QUIT...

        while (GetMessage(&msg, NULL, 0, 0))
            ;

        if (hook)
            UnhookWindowsHookEx(hook);

        target_wnd = 0;

        return 0;
    }

    int win32_enable_lowlevel_keyboard(HWND hwnd)
    {
        if (!default_hook_wnd)
        {
            default_hook_wnd = hwnd;
        }
        if (!hwnd)
        {
            hwnd = default_hook_wnd;
        }
        // Only one target at a time for now
        if (thread != NULL)
        {
            if (hwnd == target_wnd)
                return 0;

            return 1;
        }

        // We create a separate thread as it is crucial that hooks are processed
        // in a timely manner.
        thread = CreateThread(NULL, 0, keyboard_thread, hwnd, 0, &thread_id);
        if (thread == NULL)
            return 1;

        return 0;
    }

    void win32_disable_lowlevel_keyboard(HWND hwnd)
    {
        if (!hwnd)
        {
            hwnd = default_hook_wnd;
        }
        if (hwnd != target_wnd)
            return;

        PostThreadMessage(thread_id, WM_QUIT, 0, 0);

        CloseHandle(thread);
        thread = NULL;
    }

    void win_stop_system_key_propagate(bool v)
    {
        stop_system_key_propagate = v;
    }

    // https://stackoverflow.com/questions/4023586/correct-way-to-find-out-if-a-service-is-running-as-the-system-user
    BOOL is_local_system()
    {
        HANDLE hToken;
        UCHAR bTokenUser[sizeof(TOKEN_USER) + 8 + 4 * SID_MAX_SUB_AUTHORITIES];
        PTOKEN_USER pTokenUser = (PTOKEN_USER)bTokenUser;
        ULONG cbTokenUser;
        SID_IDENTIFIER_AUTHORITY siaNT = SECURITY_NT_AUTHORITY;
        PSID pSystemSid;
        BOOL bSystem;

        // open process token
        if (!OpenProcessToken(GetCurrentProcess(),
                              TOKEN_QUERY,
                              &hToken))
            return FALSE;

        // retrieve user SID
        if (!GetTokenInformation(hToken, TokenUser, pTokenUser,
                                 sizeof(bTokenUser), &cbTokenUser))
        {
            CloseHandle(hToken);
            return FALSE;
        }

        CloseHandle(hToken);

        // allocate LocalSystem well-known SID
        if (!AllocateAndInitializeSid(&siaNT, 1, SECURITY_LOCAL_SYSTEM_RID,
                                      0, 0, 0, 0, 0, 0, 0, &pSystemSid))
            return FALSE;

        // compare the user SID from the token with the LocalSystem SID
        bSystem = EqualSid(pTokenUser->User.Sid, pSystemSid);

        FreeSid(pSystemSid);

        return bSystem;
    }

    void alloc_console_and_redirect()
    {
        AllocConsole();
        freopen("CONOUT$", "w", stdout);
    }

    bool is_service_running_w(LPCWSTR serviceName)
    {
        SC_HANDLE hSCManager = OpenSCManagerW(NULL, NULL, SC_MANAGER_CONNECT);
        if (hSCManager == NULL) {
            return false;
        }

        SC_HANDLE hService = OpenServiceW(hSCManager, serviceName, SERVICE_QUERY_STATUS);
        if (hService == NULL) {
            CloseServiceHandle(hSCManager);
            return false;
        }

        SERVICE_STATUS_PROCESS serviceStatus;
        DWORD bytesNeeded;
        if (!QueryServiceStatusEx(hService, SC_STATUS_PROCESS_INFO, reinterpret_cast<LPBYTE>(&serviceStatus), sizeof(serviceStatus), &bytesNeeded)) {
            CloseServiceHandle(hService);
            CloseServiceHandle(hSCManager);
            return false;
        }

        bool isRunning = (serviceStatus.dwCurrentState == SERVICE_RUNNING);

        CloseServiceHandle(hService);
        CloseServiceHandle(hSCManager);

        return isRunning;
    }
} // end of extern "C"

// Remote printing 
extern "C"
{
#pragma comment(lib, "XpsPrint.lib")
#pragma warning(push)
#pragma warning(disable : 4995)

#define PRINT_XPS_CHECK_HR(hr, msg)                      \
    if (FAILED(hr))                                      \
    {                                                    \
        _com_error err(hr);                              \
        flog("%s Error: %s\n", msg, err.ErrorMessage()); \
        return -1;                                       \
    }

    int PrintXPSRawData(LPWSTR printerName, BYTE *rawData, ULONG dataSize)
    {
        BOOL isCoInitializeOk = FALSE;
        HRESULT hr = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
        if (hr == RPC_E_CHANGED_MODE)
        {
            hr = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
        }
        if (hr == S_OK)
        {
            isCoInitializeOk = TRUE;
        }
        std::shared_ptr<int> coInitGuard(nullptr, [isCoInitializeOk](int *) {
            if (isCoInitializeOk) CoUninitialize();
        });

        IXpsOMObjectFactory *xpsFactory = nullptr;
        hr = CoCreateInstance(
            __uuidof(XpsOMObjectFactory),
            nullptr,
            CLSCTX_INPROC_SERVER,
            __uuidof(IXpsOMObjectFactory),
            reinterpret_cast<LPVOID *>(&xpsFactory));
        PRINT_XPS_CHECK_HR(hr, "Failed to create XPS object factory.");
        std::shared_ptr<IXpsOMObjectFactory> xpsFactoryGuard(
            xpsFactory,
            [](IXpsOMObjectFactory *xpsFactory) {
                xpsFactory->Release();
        });

        HANDLE completionEvent = CreateEvent(nullptr, TRUE, FALSE, nullptr);
        if (completionEvent == nullptr)
        {
            flog("Failed to create completion event. Last error: %d\n", GetLastError());
            return -1;
        }
        std::shared_ptr<HANDLE> completionEventGuard(
            &completionEvent,
            [](HANDLE *completionEvent) {
                CloseHandle(*completionEvent);
        });

        IXpsPrintJob *job = nullptr;
        IXpsPrintJobStream *jobStream = nullptr;
        // `StartXpsPrintJob()` is deprecated, but we still use it for compatibility.
        // We may change to use the `Print Document Package API` in the future.
        // https://learn.microsoft.com/en-us/windows/win32/printdocs/xpsprint-functions
        hr = StartXpsPrintJob(
            printerName,
            L"Print Job 1",
            nullptr,
            nullptr,
            completionEvent,
            nullptr,
            0,
            &job,
            &jobStream,
            nullptr);
        PRINT_XPS_CHECK_HR(hr, "Failed to start XPS print job.");

        std::shared_ptr<IXpsPrintJobStream> jobStreamGuard(jobStream, [](IXpsPrintJobStream *jobStream) {
                jobStream->Release();
        });
        BOOL jobOk = FALSE;
        std::shared_ptr<IXpsPrintJob> jobGuard(job, [&jobOk](IXpsPrintJob* job) {
            if (jobOk == FALSE)
            {
                job->Cancel();
            }
            job->Release();
        });

        DWORD bytesWritten = 0;
        hr = jobStream->Write(rawData, dataSize, &bytesWritten);
        PRINT_XPS_CHECK_HR(hr, "Failed to write data to print job stream.");

        hr = jobStream->Close();
        PRINT_XPS_CHECK_HR(hr, "Failed to close print job stream.");

        // Wait about 5 minutes for the print job to complete.
        DWORD waitMillis = 300 * 1000;
        DWORD waitResult = WaitForSingleObject(completionEvent, waitMillis);
        if (waitResult != WAIT_OBJECT_0)
        {
            flog("Wait for print job completion failed. Last error: %d\n", GetLastError());
            return -1;
        }
        jobOk = TRUE;

        return 0;
    }

#pragma warning(pop)
}
