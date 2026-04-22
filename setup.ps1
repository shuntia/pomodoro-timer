#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Sets up GStreamer on Windows so `cargo build` works with the video feature.

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
function SetMachineEnv([string]$name, [string]$value) {
    [System.Environment]::SetEnvironmentVariable($name, $value, "Machine")
    Set-Item -Path "Env:\$name" -Value $value   # current session
    Write-Ok "$name = $value"
}

# ── 1. pkgconf (handles C:\ paths; plain pkg-config from scoop does not) ──────
Write-Step "Installing pkgconf via scoop"
if (-not (Get-Command scoop -ErrorAction SilentlyContinue)) {
    Write-Error "scoop not found. Install it from https://scoop.sh then re-run."
}
# pkgconf correctly handles Windows drive-letter paths; pkg-config (Unix build)
# splits 'C:\foo' on ':' and silently breaks all paths.
scoop install pkgconf
$pkgconfExe = (Get-Command pkgconf -ErrorAction Stop).Source
Write-Ok "pkgconf at $pkgconfExe"

# ── 2. Install GStreamer MSIs ─────────────────────────────────────────────────
Write-Step "Installing GStreamer MSIs"
foreach ($msi in @($RuntimeMsi, $DevelMsi)) {
    $msi = (Resolve-Path $msi).Path
    Write-Host "    Installing $msi ..."
    $proc = Start-Process msiexec.exe -ArgumentList "/i `"$msi`" /passive /norestart" -Wait -PassThru
    if ($proc.ExitCode -notin @(0, 3010)) {
        Write-Error "msiexec failed for $msi (exit code $($proc.ExitCode))"
    }
    Write-Ok "Installed $msi"
}

# Sanity-check: pkgconfig dir must exist after install
$gstPkgCfg = "$GstRoot\lib\pkgconfig"
if (-not (Test-Path "$gstPkgCfg\gstreamer-1.0.pc")) {
    Write-Error "GStreamer pkgconfig not found at $gstPkgCfg — check the install path."
}

# ── 3. Environment variables ──────────────────────────────────────────────────
Write-Step "Setting environment variables (Machine scope)"

# Tell cargo's pkg-config crate to use pkgconf instead of pkg-config.
# pkgconf understands ';'-separated paths and Windows drive letters.
SetMachineEnv "PKG_CONFIG" $pkgconfExe

# GStreamer root (gstreamer-sys also reads this directly on MSVC Windows).
SetMachineEnv "GSTREAMER_1_0_ROOT_MSVC_X86_64" $GstRoot

# PKG_CONFIG_PATH — merge with any existing entries, deduplicate.
$existingPkg = [System.Environment]::GetEnvironmentVariable("PKG_CONFIG_PATH", "Machine")
$pkgParts    = @($gstPkgCfg) + ($existingPkg -split ';' | Where-Object { $_ -ne '' -and $_ -ne $gstPkgCfg })
$newPkg      = ($pkgParts | Select-Object -Unique) -join ';'
SetMachineEnv "PKG_CONFIG_PATH" $newPkg

# PATH — prepend GStreamer bin so the DLLs are found at runtime.
$gstBin       = "$GstRoot\bin"
$existingPath = [System.Environment]::GetEnvironmentVariable("Path", "Machine")
$pathParts    = @($gstBin) + ($existingPath -split ';' | Where-Object { $_ -ne '' -and $_ -ne $gstBin })
$newPath      = ($pathParts | Select-Object -Unique) -join ';'
SetMachineEnv "Path" $newPath

# ── 4. Verify pkg-config can see GStreamer ────────────────────────────────────
Write-Step "Verifying pkgconf finds gstreamer-1.0"
$env:PKG_CONFIG_PATH = $newPkg   # ensure current session has the merged value
& $pkgconfExe --modversion gstreamer-1.0
if ($LASTEXITCODE -ne 0) {
    Write-Error "pkgconf still cannot find gstreamer-1.0 — check $gstPkgCfg"
}
Write-Ok "pkgconf finds gstreamer-1.0"

Write-Host ""
Write-Host "Done! Run: cargo build" -ForegroundColor Green
