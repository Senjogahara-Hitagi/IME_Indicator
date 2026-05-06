param(
    [switch]$Console,
    [switch]$Build,
    [switch]$Release,
    [switch]$Debug,
    [switch]$StopOnly
)

$ErrorActionPreference = "Stop"

if ($Release -and $Debug) {
    throw "Use either -Release or -Debug, not both."
}

$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Definition
$RustDir = Join-Path $ProjectRoot "rust_indicator"
$AppName = "IME-Indicator"
$DefaultProfile = "release"
$ProfileDir = if ($Debug) { "debug" } elseif ($Release) { "release" } else { $DefaultProfile }
$ExePath = Join-Path $RustDir "target\$ProfileDir\$AppName.exe"

Set-Location $RustDir

function Get-CargoArgs {
    param(
        [string]$Command
    )

    $args = @($Command)
    if ($ProfileDir -eq "release") {
        $args += "--release"
    }

    return ,$args
}

Get-Process $AppName -ErrorAction SilentlyContinue | Stop-Process -Force

if ($StopOnly) {
    exit 0
}

if ($Build -or -not (Test-Path $ExePath)) {
    & cargo @(Get-CargoArgs -Command "build")
}

if ($Console) {
    & cargo @(Get-CargoArgs -Command "run")
} else {
    Start-Process -FilePath $ExePath -WorkingDirectory $RustDir -WindowStyle Hidden
}
