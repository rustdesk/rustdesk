param(
  [Parameter(Mandatory)] [string]$Executable,
  [ValidateSet('port-forward', 'file-transfer', 'rdp', 'connect', 'view-camera', 'terminal')] [string]$Session = 'port-forward',
  [switch]$AlsoOpenFileTransfer,
  [switch]$AlsoOpenSamePeerCamera,
  [switch]$CloseFirstSession,
  [switch]$ReopenClosedSession,
  [string]$PeerEndpoint = '127.0.0.1:1',
  [switch]$RequireVideoFrame
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
  ArgumentList = @('--experimental-windowing', '--no-server', "--$Session", $PeerEndpoint)
  WindowStyle = 'Hidden'
  PassThru = $true
}
$stdout = $null
if ($RequireVideoFrame) {
  if ($Session -ne 'connect' -or !$env:RUSTDESK_TEST_PEER_PASSWORD) {
    throw 'Video verification requires a remote desktop session and RUSTDESK_TEST_PEER_PASSWORD.'
  }
  if ($AlsoOpenFileTransfer -or $AlsoOpenSamePeerCamera) {
    throw 'Run video verification with one connection.'
  }
  $stdout = [IO.Path]::GetTempFileName()
  $options.RedirectStandardOutput = $stdout
  $options.ArgumentList += @('--password', $env:RUSTDESK_TEST_PEER_PASSWORD)
}
$process = $null
try {
  $process = Start-Process @options
  $title = switch ($Session) {
    'file-transfer' { '*File Transfer*' }
    'connect' { '*Remote Desktop*' }
    'view-camera' { '*View Camera*' }
    'terminal' { '*Terminal*' }
    default { '*Port Forward*' }
  }
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
  if ($RequireVideoFrame) {
    $deadline = (Get-Date).AddSeconds(30)
    do {
      if ($process.HasExited) { throw "RustDesk exited before receiving video ($($process.ExitCode))." }
      $firstFrame = Select-String -LiteralPath $stdout -Pattern 'EventToUI_Texture display:' -Quiet
      if ($firstFrame) { break }
      Start-Sleep -Milliseconds 300
    } while ((Get-Date) -lt $deadline)
    if (!$firstFrame) { throw 'RustDesk did not deliver a decoded texture frame to Flutter.' }
    Write-Output 'RustDesk delivered a decoded texture frame to Flutter.'
  }
  if ($AlsoOpenFileTransfer -or $AlsoOpenSamePeerCamera) {
    if ($AlsoOpenFileTransfer -and $AlsoOpenSamePeerCamera) { throw 'Choose one second session.' }
    $secondArgs = if ($AlsoOpenSamePeerCamera) { @('--view-camera', '127.0.0.1:1') } else { @('--file-transfer', '127.0.0.1:2') }
    $secondTitle = if ($AlsoOpenSamePeerCamera) { '*View Camera*' } else { '*File Transfer*' }
    if ($title -eq $secondTitle) { throw 'Start with a different session type.' }
    $second = Start-Process -FilePath $exe -WorkingDirectory $options.WorkingDirectory -WindowStyle Hidden -PassThru -ArgumentList $secondArgs
    try {
      $deadline = (Get-Date).AddSeconds(40)
      do {
        if ($process.HasExited) {
          throw "RustDesk crashed or exited opening a second session ($($process.ExitCode))."
        }
        $windows = @((& $nativeProbe -AppProcessId $process.Id | ConvertFrom-Json))
        $children = @($windows | Where-Object {
          $_.className -eq 'FLUTTER_HOST_WINDOW' -and $_.visible -and
            ($_.title -like $title -or $_.title -like $secondTitle)
        })
        if ($children.Count -eq 2) { break }
        Start-Sleep -Milliseconds 300
      } while ((Get-Date) -lt $deadline)
      if ($children.Count -ne 2) { throw 'Expected two visible official session windows in one process.' }
      if ($process.WaitForExit(2000)) { throw "RustDesk exited with two active session windows ($($process.ExitCode))." }
      Write-Output "RustDesk kept both official session windows open."
      $secondWindow = $children | Where-Object { $_.title -like $secondTitle } | Select-Object -First 1
      if (!$secondWindow) { throw "Expected a second window matching $secondTitle." }
      $closing = if ($CloseFirstSession) { $official } else { $secondWindow }
      $remaining = if ($CloseFirstSession) { $secondWindow } else { $official }
      & $nativeProbe -AppProcessId $process.Id -Title $closing.title -RequestClose | Out-Null
      $deadline = (Get-Date).AddSeconds(15)
      do {
        if ($process.HasExited) { throw "Closing a session exited RustDesk ($($process.ExitCode))." }
        $windows = @((& $nativeProbe -AppProcessId $process.Id | ConvertFrom-Json))
        if (!($windows | Where-Object { $_.handle -eq $closing.handle -and $_.visible })) { break }
        Start-Sleep -Milliseconds 300
      } while ((Get-Date) -lt $deadline)
      if (($windows | Where-Object { $_.handle -eq $closing.handle -and $_.visible }) -or
          !($windows | Where-Object { $_.handle -eq $remaining.handle -and $_.visible })) {
        throw 'Closing one session did not leave the other running independently.'
      }
      Write-Output "Closed '$($closing.title)' and kept '$($remaining.title)' open."
      if ($ReopenClosedSession) {
        $reopenArgs = if ($CloseFirstSession) { @("--$Session", $PeerEndpoint) } else { $secondArgs }
        $relaunch = Start-Process -FilePath $exe -WorkingDirectory $options.WorkingDirectory -WindowStyle Hidden -PassThru -ArgumentList $reopenArgs
        try {
          $deadline = (Get-Date).AddSeconds(15)
          do {
            if ($process.HasExited) { throw "Reopening the session exited RustDesk ($($process.ExitCode))." }
            $windows = @((& $nativeProbe -AppProcessId $process.Id | ConvertFrom-Json))
            $reopened = $windows | Where-Object {
              $_.className -eq 'FLUTTER_HOST_WINDOW' -and $_.visible -and $_.title -eq $closing.title
            } | Select-Object -First 1
            if ($reopened) { break }
            Start-Sleep -Milliseconds 300
          } while ((Get-Date) -lt $deadline)
          if (!$reopened -or !($windows | Where-Object { $_.handle -eq $remaining.handle -and $_.visible })) {
            throw 'The closed session could not reopen alongside its sibling.'
          }
          Write-Output "Reopened '$($closing.title)' and kept '$($remaining.title)' open."
        } finally {
          if (!$relaunch.HasExited) { Stop-Process -Id $relaunch.Id }
        }
      }
    } finally {
      if (!$second.HasExited) { Stop-Process -Id $second.Id }
    }
  }
} finally {
  if ($process -and !$process.HasExited) {
    Stop-Process -Id $process.Id
    $process.WaitForExit()
  }
  if ($stdout) { Remove-Item -LiteralPath $stdout }
}
