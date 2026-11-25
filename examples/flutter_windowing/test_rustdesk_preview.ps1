param(
  [Parameter(Mandatory)] [string]$Executable,
  [ValidateSet('port-forward', 'file-transfer', 'rdp')] [string]$Session = 'port-forward',
  [switch]$AlsoOpenFileTransfer
)

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
  ArgumentList = @('--experimental-windowing', '--no-server', "--$Session", '127.0.0.1:1')
  WindowStyle = 'Hidden'
  PassThru = $true
}
$process = Start-Process @options
try {
  $title = if ($Session -eq 'file-transfer') { '*File Transfer*' } else { '*Port Forward*' }
  $deadline = (Get-Date).AddSeconds(40)
  do {
    if ($process.HasExited) {
      throw "RustDesk exited before opening the official $Session window ($($process.ExitCode))."
    }
    $windows = @((& $nativeProbe -AppProcessId $process.Id | ConvertFrom-Json))
    $official = $windows | Where-Object {
      $_.className -eq 'FLUTTER_HOST_WINDOW' -and $_.visible -and $_.title -like $title
    } | Select-Object -First 1
    if ($official) { break }
    Start-Sleep -Milliseconds 300
  } while ((Get-Date) -lt $deadline)
  if (!$official) {
    $windows | ConvertTo-Json -Compress
    throw "No visible SDK-owned $Session window opened in the RustDesk process."
  }
  Write-Output "RustDesk opened HWND $($official.handle) of class $($official.className)."
  if ($AlsoOpenFileTransfer) {
    if ($Session -eq 'file-transfer') { throw 'Start with a TCP forwarding or RDP window.' }
    $second = Start-Process -FilePath $exe -WorkingDirectory $options.WorkingDirectory -WindowStyle Hidden -PassThru -ArgumentList @('--file-transfer', '127.0.0.1:2')
    try {
      $deadline = (Get-Date).AddSeconds(40)
      do {
        if ($process.HasExited) {
          throw "RustDesk crashed or exited opening a second session ($($process.ExitCode))."
        }
        $windows = @((& $nativeProbe -AppProcessId $process.Id | ConvertFrom-Json))
        $children = @($windows | Where-Object {
          $_.className -eq 'FLUTTER_HOST_WINDOW' -and $_.visible -and
            ($_.title -like '*Port Forward*' -or $_.title -like '*File Transfer*')
        })
        if ($children.Count -eq 2) { break }
        Start-Sleep -Milliseconds 300
      } while ((Get-Date) -lt $deadline)
      if ($children.Count -ne 2) { throw 'Expected two visible official session windows in one process.' }
      if ($process.WaitForExit(2000)) { throw "RustDesk exited with two active session windows ($($process.ExitCode))." }
      Write-Output "RustDesk kept both official session windows open."
      $fileWindow = $children | Where-Object { $_.title -like '*File Transfer*' } | Select-Object -First 1
      if (!$fileWindow) { throw 'Expected a file transfer window.' }
      & $nativeProbe -AppProcessId $process.Id -Title $fileWindow.title -RequestClose | Out-Null
      $deadline = (Get-Date).AddSeconds(15)
      do {
        if ($process.HasExited) { throw "Closing file transfer exited RustDesk ($($process.ExitCode))." }
        $windows = @((& $nativeProbe -AppProcessId $process.Id | ConvertFrom-Json))
        if (!($windows | Where-Object { $_.handle -eq $fileWindow.handle -and $_.visible })) { break }
        Start-Sleep -Milliseconds 300
      } while ((Get-Date) -lt $deadline)
      if (($windows | Where-Object { $_.handle -eq $fileWindow.handle -and $_.visible }) -or
          !($windows | Where-Object { $_.handle -eq $official.handle -and $_.visible })) {
        throw 'Closing file transfer did not leave the forwarding window running independently.'
      }
      Write-Output "Closing file transfer left the forwarding window open."
    } finally {
      if (!$second.HasExited) { Stop-Process -Id $second.Id }
    }
  }
} finally {
  if (!$process.HasExited) {
    Stop-Process -Id $process.Id
    $process.WaitForExit()
  }
}
