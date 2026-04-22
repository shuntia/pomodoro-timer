#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Sets up GStreamer on Windows so `cargo build` works with the video feature.
.DESCRIPTION
    1. Ensures pkg-config is available (via scoop)
    2. Downloads GStreamer runtime + devel MSIs (MSVC 64-bit)
    3. Installs them silently
    4. Sets GSTREAMER_1_0_ROOT_MSVC_X86_64, PKG_CONFIG_PATH, and PATH
       at the Machine level so all future shells pick them up
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ── Config ────────────────────────────────────────────────────────────────────
$GstVersion  = "1.24.10"
$GstArch     = "x86_64"
$GstRoot     = "C:\gstreamer\1.0\msvc_$GstArch"
$GstBase     = "https://gstreamer.freedesktop.org/data/pkg/windows/$GstVersion/msvc"
$RuntimeMsi  = "gstreamer-1.0-msvc-$GstArch-$GstVersion.msi"
$DevelMsi    = "gstreamer-1.0-devel-msvc-$GstArch-$GstVersion.msi"
$Tmp         = "$env:TEMP\gstreamer-setup"
# ─────────────────────────────────────────────────────────────────────────────

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "    OK: $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "    WARN: $msg" -ForegroundColor Yellow }

# ── 1. pkg-config via scoop ───────────────────────────────────────────────────
Write-Step "Checking pkg-config"
if (Get-Command pkg-config -ErrorAction SilentlyContinue) {
    Write-Ok "pkg-config already installed"
} else {
    if (-not (Get-Command scoop -ErrorAction SilentlyContinue)) {
        Write-Error "scoop not found. Install it from https://scoop.sh then re-run this script."
    }
    Write-Host "    Installing pkg-config via scoop..."
    scoop install pkg-config
    Write-Ok "pkg-config installed"
}

# ── 2. Download MSIs ──────────────────────────────────────────────────────────
Write-Step "Downloading GStreamer $GstVersion MSIs"
New-Item -ItemType Directory -Force -Path $Tmp | Out-Null

foreach ($msi in @($RuntimeMsi, $DevelMsi)) {
    $dest = Join-Path $Tmp $msi
    if (Test-Path $dest) {
        Write-Ok "$msi already downloaded"
    } else {
        $url = "$GstBase/$msi"
        Write-Host "    Downloading $url ..."
        Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing
        Write-Ok "Downloaded $msi"
    }
}

# ── 3. Install MSIs ───────────────────────────────────────────────────────────
Write-Step "Installing GStreamer (this may take a minute)"
foreach ($msi in @($RuntimeMsi, $DevelMsi)) {
    $dest = Join-Path $Tmp $msi
    Write-Host "    Installing $msi ..."
    $proc = Start-Process msiexec.exe -ArgumentList "/i `"$dest`" /passive /norestart" -Wait -PassThru
    if ($proc.ExitCode -notin @(0, 3010)) {
        Write-Error "msiexec failed for $msi (exit code $($proc.ExitCode))"
    }
    Write-Ok "Installed $msi"
}

# ── 4. Environment variables ──────────────────────────────────────────────────
Write-Step "Setting environment variables (Machine scope)"

$gstBin     = "$GstRoot\bin"
$gstPkgCfg  = "$GstRoot\lib\pkgconfig"

# GSTREAMER_1_0_ROOT_MSVC_X86_64
[System.Environment]::SetEnvironmentVariable(
    "GSTREAMER_1_0_ROOT_MSVC_X86_64", $GstRoot, "Machine")
Write-Ok "GSTREAMER_1_0_ROOT_MSVC_X86_64 = $GstRoot"

# PKG_CONFIG_PATH — merge with any existing value, deduplicate
$existingPkg = [System.Environment]::GetEnvironmentVariable("PKG_CONFIG_PATH", "Machine")
$pkgParts    = @($gstPkgCfg) + ($existingPkg -split ';' | Where-Object { $_ -ne '' -and $_ -ne $gstPkgCfg })
$newPkg      = ($pkgParts | Select-Object -Unique) -join ';'
[System.Environment]::SetEnvironmentVariable("PKG_CONFIG_PATH", $newPkg, "Machine")
Write-Ok "PKG_CONFIG_PATH = $newPkg"

# PATH — prepend GStreamer bin
$existingPath = [System.Environment]::GetEnvironmentVariable("Path", "Machine")
$pathParts    = @($gstBin) + ($existingPath -split ';' | Where-Object { $_ -ne '' -and $_ -ne $gstBin })
$newPath      = ($pathParts | Select-Object -Unique) -join ';'
[System.Environment]::SetEnvironmentVariable("Path", $newPath, "Machine")
Write-Ok "PATH prepended with $gstBin"

# Also apply to current session so you don't need to open a new shell
$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = $GstRoot
$env:PKG_CONFIG_PATH = $newPkg
$env:Path = $newPath

# ── Done ──────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "Setup complete!" -ForegroundColor Green
Write-Host "You can now run:  cargo build" -ForegroundColor Green
Write-Host "(No need to open a new terminal — env vars are already active in this session.)"
