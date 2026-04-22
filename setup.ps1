#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Sets up GStreamer on Windows so `cargo build` works with the video feature.
.DESCRIPTION
    Installs GStreamer runtime + devel MSIs (already downloaded) and configures
    GSTREAMER_1_0_ROOT_MSVC_X86_64, PKG_CONFIG_PATH, and PATH at Machine scope.

.PARAMETER RuntimeMsi
    Path to gstreamer-1.0-msvc-x86_64-*.msi  (runtime installer)
.PARAMETER DevelMsi
    Path to gstreamer-1.0-devel-msvc-x86_64-*.msi  (development installer)

.EXAMPLE
    .\setup.ps1 -RuntimeMsi .\gstreamer-1.0-msvc-x86_64-1.24.10.msi `
                -DevelMsi   .\gstreamer-1.0-devel-msvc-x86_64-1.24.10.msi
#>
param(
    [Parameter(Mandatory)][string]$RuntimeMsi,
    [Parameter(Mandatory)][string]$DevelMsi
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$GstArch = "x86_64"
$GstRoot = "C:\gstreamer\1.0\msvc_$GstArch"

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "    OK: $msg" -ForegroundColor Green }

# ── 1. pkg-config via scoop ───────────────────────────────────────────────────
Write-Step "Checking pkg-config"
if (Get-Command pkg-config -ErrorAction SilentlyContinue) {
    Write-Ok "pkg-config already installed"
} else {
    if (-not (Get-Command scoop -ErrorAction SilentlyContinue)) {
        Write-Error "scoop not found. Install it from https://scoop.sh then re-run."
    }
    Write-Host "    Installing pkg-config via scoop..."
    scoop install pkg-config
    Write-Ok "pkg-config installed"
}

# ── 2. Install MSIs ───────────────────────────────────────────────────────────
Write-Step "Installing GStreamer MSIs"
foreach ($msi in @($RuntimeMsi, $DevelMsi)) {
    $msi = Resolve-Path $msi   # error early if path is wrong
    Write-Host "    Installing $msi ..."
    $proc = Start-Process msiexec.exe -ArgumentList "/i `"$msi`" /passive /norestart" -Wait -PassThru
    if ($proc.ExitCode -notin @(0, 3010)) {
        Write-Error "msiexec failed for $msi (exit code $($proc.ExitCode))"
    }
    Write-Ok "Installed $msi"
}

# ── 3. Environment variables ──────────────────────────────────────────────────
Write-Step "Setting environment variables (Machine scope)"

$gstBin    = "$GstRoot\bin"
$gstPkgCfg = "$GstRoot\lib\pkgconfig"

[System.Environment]::SetEnvironmentVariable(
    "GSTREAMER_1_0_ROOT_MSVC_X86_64", $GstRoot, "Machine")
Write-Ok "GSTREAMER_1_0_ROOT_MSVC_X86_64 = $GstRoot"

$existingPkg = [System.Environment]::GetEnvironmentVariable("PKG_CONFIG_PATH", "Machine")
$pkgParts    = @($gstPkgCfg) + ($existingPkg -split ';' | Where-Object { $_ -ne '' -and $_ -ne $gstPkgCfg })
$newPkg      = ($pkgParts | Select-Object -Unique) -join ';'
[System.Environment]::SetEnvironmentVariable("PKG_CONFIG_PATH", $newPkg, "Machine")
Write-Ok "PKG_CONFIG_PATH = $newPkg"

$existingPath = [System.Environment]::GetEnvironmentVariable("Path", "Machine")
$pathParts    = @($gstBin) + ($existingPath -split ';' | Where-Object { $_ -ne '' -and $_ -ne $gstBin })
$newPath      = ($pathParts | Select-Object -Unique) -join ';'
[System.Environment]::SetEnvironmentVariable("Path", $newPath, "Machine")
Write-Ok "PATH prepended with $gstBin"

# Apply to current session immediately
$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = $GstRoot
$env:PKG_CONFIG_PATH                 = $newPkg
$env:Path                            = $newPath

Write-Host ""
Write-Host "Done! Run: cargo build" -ForegroundColor Green
