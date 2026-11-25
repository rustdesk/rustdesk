param(
  [Parameter(Mandatory)] [string]$FlutterSdk,
  [switch]$SelfTest
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$flutter = Join-Path (Resolve-Path $FlutterSdk).Path 'bin/flutter.bat'
$dart = Join-Path (Resolve-Path $FlutterSdk).Path 'bin/dart.bat'
$drive = @('Z','Y','X','W','V','U','T','S') | Where-Object { !(Test-Path "${_}:\") } | Select-Object -First 1
if (!$drive) { throw 'No unused drive letter for the Windows short-path build.' }
$oldWindowing = $env:FLUTTER_WINDOWING
$oldResults = $env:POC_RESULTS
& subst "${drive}:" $repo
if ($LASTEXITCODE) { throw 'Could not create temporary short-path mapping.' }
Push-Location "${drive}:\experimental_example"
try {
  $env:FLUTTER_WINDOWING = 'true'
  $dartVersion = & $dart --version 2>&1 | Out-String
  if ($LASTEXITCODE -or $dartVersion -notmatch 'windows_(x64|arm64)') {
    throw "Could not determine the native Dart SDK architecture: $dartVersion"
  }
  $arch = $Matches[1]
  if ($env:FLUTTER_HOST_ARCH -and $arch -ne $env:FLUTTER_HOST_ARCH) {
    throw "Expected $env:FLUTTER_HOST_ARCH Dart, got $arch."
  }
  & $flutter clean
  if ($LASTEXITCODE) { throw 'Could not clear the generated example build.' }
  & $flutter pub get
  if ($LASTEXITCODE) { throw 'Dependency resolution failed.' }
  $mode = if ($SelfTest) { 'debug' } else { 'release' }
  [string[]]$buildArguments = @('build', 'windows', "--$mode")
  if ($SelfTest) { $buildArguments += '--dart-define=POC_SELF_TEST=true' }
  & $flutter @buildArguments
  if ($LASTEXITCODE) { throw 'Windows build failed.' }
  $exe = Join-Path $PWD "build/windows/$arch/runner/$mode/windowing_poc.exe"
  if ($SelfTest) {
    $selfTests = @(
      @{ Arguments = @(); Report = 'poc-results.json' },
      @{ Arguments = @('--implicit-main-self-test'); Report = 'poc-implicit-main-results.json' }
    )
    foreach ($testCase in $selfTests) {
      $env:POC_RESULTS = Join-Path $repo "experimental_example/$($testCase.Report)"
      if (Test-Path -LiteralPath $env:POC_RESULTS) {
        Remove-Item -LiteralPath $env:POC_RESULTS
      }
      $options = @{ FilePath = $exe; WorkingDirectory = $PWD; WindowStyle = 'Hidden'; PassThru = $true }
      if ($testCase.Arguments.Count) { $options.ArgumentList = $testCase.Arguments }
      $process = Start-Process @options
      if (!$process.WaitForExit(60000)) {
        Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
        throw "PoC timed out: $($testCase.Report)"
      }
      if (Test-Path -LiteralPath $env:POC_RESULTS) {
        Get-Content -LiteralPath $env:POC_RESULTS
      }
      if ($process.ExitCode -ne 0) {
        throw "PoC exited with code $($process.ExitCode): $($testCase.Report)"
      }
      if (!(Test-Path -LiteralPath $env:POC_RESULTS)) {
        throw "PoC exited without writing $($testCase.Report)"
      }
    }
  } else {
    $process = Start-Process -FilePath $exe -WorkingDirectory $PWD -WindowStyle Normal -PassThru
    $process.WaitForExit()
    if ($process.ExitCode) { throw "PoC exited with code $($process.ExitCode)" }
  }
} finally {
  Pop-Location
  $env:FLUTTER_WINDOWING = $oldWindowing
  $env:POC_RESULTS = $oldResults
  & subst "${drive}:" /d
}
