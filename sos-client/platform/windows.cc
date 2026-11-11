// SOS 便携服务使用的 Windows C++ 辅助函数
// 精简自 src/platform/windows.cc

#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif

#include <windows.h>
#include <wtsapi32.h>
#include <tlhelp32.h>

#pragma comment(lib, "wtsapi32.lib")

extern "C" {

// ── SYSTEM 令牌 ──

/// 以 SYSTEM 身份启动进程，**可优先选控制台会话**
/// 等效于上游的 impersonate_system::run_as_system。
///
/// cmd: 完整的命令行（含 exe 路径和参数）
/// preferConsole: 是否优先把子进程放到控制台会话。
///                ⚠ 仅当跨会话 IPC（`Global\` 内核对象）可用时才能为真，
///                否则子进程会连不上共享内存 / 命名事件。
/// outSession/outPid: 可选，回传选中的 winlogon 会话与 PID（诊断用）
/// outConsoleSession: 可选，回传控制台会话 id（诊断用，0xFFFFFFFF = 无）
/// 返回进程句柄（调用者负责 CloseHandle），失败返回 0（NULL）。
intptr_t SosRunAsSystemInSession1(LPCWSTR cmd, BOOL preferConsole, DWORD* outSession,
                                  DWORD* outPid, DWORD* outConsoleSession) {
    if (outSession) *outSession = 0;
    if (outPid) *outPid = 0;
    if (outConsoleSession) *outConsoleSession = 0xFFFFFFFF;

    // 启用 SeDebugPrivilege
    HANDLE hToken = NULL;
    if (OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &hToken)) {
        TOKEN_PRIVILEGES tp;
        tp.PrivilegeCount = 1;
        tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;
        LookupPrivilegeValueW(NULL, L"SeDebugPrivilege", &tp.Privileges[0].Luid);
        AdjustTokenPrivileges(hToken, FALSE, &tp, sizeof(tp), NULL, NULL);
        CloseHandle(hToken);
    }

    // ── 选择目标会话 ──
    //
    // 优先级：**控制台会话**（仅当 preferConsole）> 本进程所在会话 > 任意非 0 会话。
    //
    // 为什么优先控制台会话：现场的使用方式是「在 RDP 会话里启动 SOS →
    // 退出 RDP → 从控制端连接」。退出 RDP 后那个会话变成 `WTSDisconnected`，
    // Windows **不再为它合成桌面**，于是 BitBlt / PrintWindow / DXGI
    // 全部拿不到像素（现场日志：E_ACCESSDENIED 持续 30 秒，直到重新接入
    // RDP 的那一瞬间才成功）。而控制台会话始终绑定物理显示输出，即使
    // 没有人登录（停在登录界面）也仍然在合成画面，因此是可采集的。
    //
    // 上游在 `display_service.rs` 也承认了断开会话无法解决：
    //   "The virtual display cannot be added when there's no session
    //    (eg. when exiting from RDP)."
    DWORD consoleSession = WTSGetActiveConsoleSessionId();
    if (outConsoleSession) *outConsoleSession = consoleSession;
    if (!preferConsole) consoleSession = 0xFFFFFFFF; // 不允许跨会话 → 只在本会话里找

    DWORD mySession = 0;
    ProcessIdToSessionId(GetCurrentProcessId(), &mySession);

    // 找 winlogon：按 控制台 → 本会话 → 任意 的顺序逐级降级
    HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snapshot == INVALID_HANDLE_VALUE) return 0;
    DWORD winlogonPid = 0;
    DWORD winlogonSession = 0;
    DWORD myPid = 0;
    DWORD mySessionFound = 0;
    DWORD anyPid = 0;
    DWORD anySession = 0;
    PROCESSENTRY32W pe = { sizeof(PROCESSENTRY32W) };
    if (Process32FirstW(snapshot, &pe)) {
        do {
            if (_wcsicmp(pe.szExeFile, L"winlogon.exe") != 0) continue;
            DWORD sessionId = 0;
            if (!ProcessIdToSessionId(pe.th32ProcessID, &sessionId) || sessionId == 0) continue;
            if (consoleSession != 0xFFFFFFFF && sessionId == consoleSession && winlogonPid == 0) {
                winlogonPid = pe.th32ProcessID;
                winlogonSession = sessionId;
            }
            if (mySession != 0 && sessionId == mySession && myPid == 0) {
                myPid = pe.th32ProcessID;
                mySessionFound = sessionId;
            }
            if (anyPid == 0) {
                anyPid = pe.th32ProcessID;
                anySession = sessionId;
            }
        } while (Process32NextW(snapshot, &pe));
    }
    CloseHandle(snapshot);

    if (winlogonPid == 0) {
        winlogonPid = myPid;
        winlogonSession = mySessionFound;
    }
    if (winlogonPid == 0) {
        winlogonPid = anyPid;
        winlogonSession = anySession;
    }
    if (winlogonPid == 0) return 0;

    // 打开 winlogon 进程
    HANDLE hProcess = OpenProcess(PROCESS_ALL_ACCESS, TRUE, winlogonPid);
    if (!hProcess) return 0;

    // 打开其令牌
    HANDLE TokenHandle = NULL;
    if (!OpenProcessToken(hProcess, TOKEN_DUPLICATE | TOKEN_ASSIGN_PRIMARY | TOKEN_QUERY, &TokenHandle)) {
        CloseHandle(hProcess);
        return 0;
    }
    CloseHandle(hProcess);

    // ImpersonateLoggedOnUser（与上游 StealToken 一致）
    //
    // 注意：模拟是**线程级**的。这里拿到令牌后必须立刻撤销，否则
    // 调用线程会在本函数返回后继续以 winlogon 的 SYSTEM 身份运行，
    // 后续所有调度到这个线程上的代码（主线程同时也是 tokio worker）
    // 都会带着这个身份执行。对用户态进程来说，这是安全软件 HIPS 里
    // 最高等级的告警特征（令牌窃取），会招来持续的行为监控。
    ImpersonateLoggedOnUser(TokenHandle);

    // 复制令牌为主令牌
    HANDLE NewToken = NULL;
    if (!DuplicateTokenEx(TokenHandle, TOKEN_ALL_ACCESS, NULL, SecurityImpersonation, TokenPrimary, &NewToken)) {
        CloseHandle(TokenHandle);
        RevertToSelf();
        return 0;
    }
    CloseHandle(TokenHandle);

    // 主令牌已复制完成，模拟身份不再需要 —— 立即撤销。
    // 下面只用 NewToken 创建进程，不依赖当前线程的模拟状态。
    RevertToSelf();

    // 用该令牌创建进程
    STARTUPINFOW si = {0};
    PROCESS_INFORMATION pi = {0};
    si.cb = sizeof(si);
    si.lpDesktop = (LPWSTR)L"winsta0\\default";

    // 获取当前目录作为进程工作目录
    wchar_t NPath[MAX_PATH];
    if (GetCurrentDirectory(MAX_PATH, NPath) == 0) {
        wcscpy_s(NPath, MAX_PATH, L"C:\\");
    }

    // 复制 cmd（CreateProcessWithTokenW 需要可写缓冲区）
    wchar_t cmdBuf[32768];
    wcscpy_s(cmdBuf, cmd);

    BOOL result = CreateProcessWithTokenW(
        NewToken,
        LOGON_WITH_PROFILE,
        NULL,           // 使用命令行中的 exe
        cmdBuf,
        0,              // 无特殊创建标志
        NULL,
        NPath,
        &si,
        &pi
    );

    CloseHandle(NewToken);

    if (!result) {
        return 0;
    }

    if (outSession) *outSession = winlogonSession;
    if (outPid) *outPid = winlogonPid;

    // 关闭线程句柄，保留进程句柄返回给调用者用于 WaitForSingleObject 监控
    CloseHandle(pi.hThread);
    return (intptr_t)pi.hProcess;
}

// ── 输入桌面切换 ──

/// 当前线程所在桌面的句柄缓存。
///
/// **不能关闭 `GetThreadDesktop()` 返回的句柄** —— MSDN 明确写明该句柄由
/// 系统拥有，关闭它是未定义行为。旧实现在切换后 `CloseDesktop(old_desktop)`，
/// 而 `old_desktop` 恰恰就是 `GetThreadDesktop()` 的结果。现场日志里
/// `BitBlt` 在切换后连续 900 次返回 `E_ACCESSDENIED`（期间自检过一次成功），
/// 与「线程桌面引用被打断后 GDI 无法再定位该桌面的显示表面」一致。
///
/// 现在只关闭**我们自己 `OpenDesktop`/`OpenInputDesktop` 打开**的句柄，
/// 并把它记在下面这个静态变量里，下次切换时再释放上一个。
static HDESK s_hHeldDesktop = NULL;

static int SwitchToDesktopHandle(HDESK desktop) {
    if (!desktop) return 0;
    if (!SetThreadDesktop(desktop)) {
        CloseDesktop(desktop);
        return 0;
    }
    // SetThreadDesktop 成功后线程已持有引用；这里保留自己的句柄不关，
    // 下次切换到别的桌面时再释放，避免「开关同一次」造成的时序问题。
    if (s_hHeldDesktop && s_hHeldDesktop != desktop) {
        CloseDesktop(s_hHeldDesktop);
    }
    s_hHeldDesktop = desktop;
    return 1;
}

/// 切换调用线程到输入桌面（安全桌面：锁屏 Winlogon / UAC）。
///
/// 访问掩码与上游 `src/platform/windows.cc::selectInputDesktop()` 一致。
/// 之前这里只请求了 `DESKTOP_READOBJECTS | DESKTOP_SWITCHDESKTOP`，
/// `SetThreadDesktop` 能成功，但随后拿到的 GDI DC 在新桌面上做 BitBlt
/// 会失败/读到旧桌面的内容 —— 就是「切过去但画面还是冻结的」。
///
/// 调用前需确保线程已 impersonate SYSTEM 或有足够权限。
/// 返回 1=成功，0=失败
int SosSwitchToInputDesktop() {
    HDESK desktop = OpenInputDesktop(0, FALSE,
        DESKTOP_CREATEMENU | DESKTOP_CREATEWINDOW | DESKTOP_ENUMERATE |
        DESKTOP_HOOKCONTROL | DESKTOP_WRITEOBJECTS | DESKTOP_READOBJECTS |
        DESKTOP_SWITCHDESKTOP | GENERIC_WRITE);
    return SwitchToDesktopHandle(desktop);
}

// ── 桌面切换事件监控 ──

static HANDLE s_hDesktopSwitchEvent = NULL;
static HWINEVENTHOOK s_hDesktopSwitchHook = NULL;

/// 每次等待的最长时间。out-of-context 的 WinEvent 不保证唤醒
/// `MsgWaitForMultipleObjects`，所以必须周期性醒来主动泵消息，
/// 否则回调永远不会被派发。250ms = 4 次/秒的唤醒，CPU 开销可忽略。
static const DWORD DESKTOP_SWITCH_POLL_SLICE = 250;

static VOID CALLBACK DesktopSwitchWinEventProc(
    HWINEVENTHOOK, DWORD, HWND, LONG, LONG, DWORD, DWORD)
{
    if (s_hDesktopSwitchEvent) {
        SetEvent(s_hDesktopSwitchEvent);
    }
}

static void EnsureDesktopSwitchHook()
{
    if (s_hDesktopSwitchHook) return;

    s_hDesktopSwitchEvent = CreateEventW(NULL, FALSE, FALSE, NULL); // auto-reset
    if (!s_hDesktopSwitchEvent) return;

    // 必须用 WINEVENT_OUTOFCONTEXT。
    //
    // WINEVENT_INCONTEXT 要求回调函数位于 **DLL** 中（系统要把它映射进
    // 产生事件的那个进程），而我们的回调在本 exe 里，实测 SetWinEventHook
    // 直接返回 NULL。注册失败后 SosWaitForDesktopSwitch 会立刻返回 -1，
    // 而调用方是 `loop { wait(); ... }` 形式的无 sleep 循环 —— 会 100% 占满
    // 一个核心（4 核机器上正好 25%），而且每轮还要做一次全进程快照。
    //
    // OUTOFCONTEXT 的事件通过本线程的消息队列派发，配合下面的消息泵即可。
    s_hDesktopSwitchHook = SetWinEventHook(
        EVENT_SYSTEM_DESKTOPSWITCH,
        EVENT_SYSTEM_DESKTOPSWITCH,
        NULL,
        DesktopSwitchWinEventProc,
        0, 0,
        WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS
    );
    if (!s_hDesktopSwitchHook) {
        CloseHandle(s_hDesktopSwitchEvent);
        s_hDesktopSwitchEvent = NULL;
    }
}

/// 阻塞等待下一次桌面切换，或 dwTimeout 毫秒超时。
/// 首次调用时注册 SetWinEventHook 并驱动消息泵。
/// 返回 1=桌面已切换，0=超时，-1=错误/WM_QUIT/钩子不可用。
///
/// 调用方必须把 -1 当作「本次没有事件」并自行限制重试频率 ——
/// 这里返回得很快，直接在循环里重试会变成忙等。
int SosWaitForDesktopSwitch(DWORD dwTimeout)
{
    EnsureDesktopSwitchHook();
    if (!s_hDesktopSwitchHook || !s_hDesktopSwitchEvent) return -1;

    DWORD start = GetTickCount();
    for (;;) {
        DWORD wait = DESKTOP_SWITCH_POLL_SLICE;
        if (dwTimeout != INFINITE) {
            DWORD elapsed = GetTickCount() - start;
            if (elapsed >= dwTimeout) return 0;
            DWORD remaining = dwTimeout - elapsed;
            if (remaining < wait) wait = remaining;
        }

        // MsgWaitForMultipleObjects: 同时等事件 + 窗口消息
        DWORD ret = MsgWaitForMultipleObjects(
            1, &s_hDesktopSwitchEvent, FALSE, wait, QS_ALLINPUT);

        if (ret == WAIT_OBJECT_0) {
            // 桌面切换事件已触发
            return 1;
        } else if (ret == WAIT_OBJECT_0 + 1 || ret == WAIT_TIMEOUT) {
            // 有窗口消息，或者只是到了这一片的时间上限 —— 两种情况都泵一次消息，
            // WinEvent 的派发就发生在这里面。
            MSG msg;
            while (PeekMessageW(&msg, NULL, 0, 0, PM_REMOVE)) {
                if (msg.message == WM_QUIT) return -1;
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            // 回调可能刚刚在泵消息时把事件置位（auto-reset，这里取走即清）
            if (WaitForSingleObject(s_hDesktopSwitchEvent, 0) == WAIT_OBJECT_0) {
                return 1;
            }
        } else {
            return -1;
        }
    }
}

} // extern "C"

