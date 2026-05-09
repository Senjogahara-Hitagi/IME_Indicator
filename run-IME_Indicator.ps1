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
$AppName = "IME_indicator"
$DefaultProfile = "release"
$ProfileDir = if ($Debug) { "debug" } elseif ($Release) { "release" } else { $DefaultProfile }
$ExePath = Join-Path $ProjectRoot "target\$ProfileDir\$AppName.exe"
$RepoConfigPath = Join-Path $RustDir "config.toml"
$RuntimeConfigPath = Join-Path $ProjectRoot "target\$ProfileDir\config.toml"

Set-Location $ProjectRoot

function Get-CargoArgs {
    param(
        [string]$Command
    )

    $args = @($Command)
    if ($ProfileDir -eq "release") {
        $args += "--release"
    }

    return $args
}

Get-Process $AppName -ErrorAction SilentlyContinue | Stop-Process -Force

if ($StopOnly) {
    exit 0
}

# Default to rebuilding before every launch so hidden restarts pick up local edits.
& cargo @(Get-CargoArgs -Command "build")

if (Test-Path $RepoConfigPath) {
    Copy-Item -LiteralPath $RepoConfigPath -Destination $RuntimeConfigPath -Force
}

if ($Console) {
    & cargo @(Get-CargoArgs -Command "run") "-p" "IME_indicator"
} else {
    Start-Process -FilePath $ExePath -WorkingDirectory $RustDir -WindowStyle Hidden
}
