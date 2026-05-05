$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RustDir = Join-Path $ProjectRoot "rust_indicator"
$AppName = "IME-Indicator"
$ExePath = Join-Path $RustDir "target\release\$AppName.exe"

Set-Location $RustDir

Write-Host "Checking for existing $AppName processes..."
Get-Process $AppName -ErrorAction SilentlyContinue | Stop-Process -Force

Write-Host "Building $AppName in release mode..."
cargo build --release

if (Test-Path $ExePath) {
    Write-Host "Successfully built $AppName."
    Write-Host "Starting $AppName (Hidden)..."
    Start-Process -FilePath $ExePath -WorkingDirectory $RustDir -WindowStyle Hidden
    Write-Host "Process started. You can find it in the system tray if enabled."
} else {
    Write-Error "Failed to find built executable at $ExePath"
}
