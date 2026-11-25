param([int]$AppProcessId, [string]$Title, [switch]$RequestClose)
$ErrorActionPreference = 'Stop'
if (!('PocNative' -as [type])) { Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class PocNative {
  public delegate bool EnumProc(IntPtr hwnd, IntPtr arg);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr arg);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder text, int count);
  [DllImport("user32.dll")] public static extern int GetWindowLong(IntPtr hwnd, int index);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr GetProp(IntPtr hwnd, string name);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr w, IntPtr l);
}
'@
}
$found = [System.Collections.Generic.List[object]]::new()
[PocNative]::EnumWindows({ param($hwnd, $arg)
  [uint32]$ownerId = 0
  [void][PocNative]::GetWindowThreadProcessId($hwnd, [ref]$ownerId)
  if ($ownerId -eq $AppProcessId) {
    $text = [System.Text.StringBuilder]::new(1024)
    [void][PocNative]::GetWindowText($hwnd, $text, 1024)
    if (!$Title -or $text.ToString() -eq $Title) {
      $class = [System.Text.StringBuilder]::new(256)
      [void][PocNative]::GetClassName($hwnd, $class, 256)
      $found.Add([pscustomobject]@{
        handle=$hwnd.ToInt64(); title=$text.ToString()
        className=$class.ToString()
        visible=[PocNative]::IsWindowVisible($hwnd)
        caption=(([PocNative]::GetWindowLong($hwnd, -16) -band 0x00C00000) -eq 0x00C00000)
        maximized=[PocNative]::IsZoomed($hwnd)
        minimized=[PocNative]::IsIconic($hwnd)
        foreground=($hwnd -eq [PocNative]::GetForegroundWindow())
        moveCommand=[PocNative]::GetProp($hwnd, 'RustDeskPocMoveRequested').ToInt64()
      })
      if ($RequestClose) { [void][PocNative]::PostMessage($hwnd, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) }
    }
  }
  return $true
}, [IntPtr]::Zero) | Out-Null
ConvertTo-Json -InputObject @($found.ToArray()) -Compress
