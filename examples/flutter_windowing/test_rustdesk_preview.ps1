param([Parameter(Mandatory)] [string]$Executable)

$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $Executable).Path
$nativeProbe = Join-Path $PSScriptRoot 'experimental_example/tool/native_probe.ps1'
if (Get-Process rustdesk -ErrorAction SilentlyContinue) {
  throw 'Run this smoke test on an idle CI desktop with no other RustDesk process.'
}

# The loopback endpoint exercises window creation without requiring a remote peer.
$options = @{
  FilePath = $exe
  WorkingDirectory = Split-Path -Parent $exe
  ArgumentList = @('--experimental-windowing', '--no-server', '--port-forward', '127.0.0.1:1')
  WindowStyle = 'Hidden'
  PassThru = $true
}
$process = Start-Process @options
try {
  $deadline = (Get-Date).AddSeconds(40)
  do {
    if ($process.HasExited) {
      throw "RustDesk exited before opening the official TCP forwarding window ($($process.ExitCode))."
    }
    $windows = @((& $nativeProbe -AppProcessId $process.Id | ConvertFrom-Json))
    $official = $windows | Where-Object {
      $_.className -eq 'FLUTTER_HOST_WINDOW' -and $_.visible -and $_.title -like '*Port Forward*'
    } | Select-Object -First 1
    if ($official) { break }
    Start-Sleep -Milliseconds 300
  } while ((Get-Date) -lt $deadline)
  if (!$official) {
    $windows | ConvertTo-Json -Compress
    throw 'No visible SDK-owned TCP forwarding window opened in the RustDesk process.'
  }
  Write-Output "RustDesk opened HWND $($official.handle) of class $($official.className)."
} finally {
  if (!$process.HasExited) {
    Stop-Process -Id $process.Id
    $process.WaitForExit()
  }
}
