#Requires -RunAsAdministrator
<#
.SYNOPSIS
    Setup script for building bagZ Desktop on Windows.

.DESCRIPTION
    Installs all required development tools via Chocolatey:
    - Visual Studio Build Tools 2022 with C++ workload
    - Rust (via rustup)
    - Bun (JavaScript runtime)
    - Protocol Buffers compiler (protoc)
    - Make (build automation)
    - OpenSSL (required for SQLCipher)

.PARAMETER Force
    If specified, allows destructive operations such as uninstalling existing
    OpenSSL installations that lack static libraries. Without this flag, the
    script will skip such operations and warn the user instead.

.NOTES
    Run this script as Administrator:
    Right-click PowerShell -> Run as Administrator
    Then: .\scripts\setup-windows.ps1

    To allow OpenSSL reinstallation if needed:
    .\scripts\setup-windows.ps1 -Force
#>

param(
    [switch]$Force
)

$ErrorActionPreference = "Stop"

# Helper function to refresh PATH from environment
function Update-PathFromEnvironment {
    $env:Path = [System.Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path", "User")
}

Write-Host "=== bagZ Windows Build Setup ===" -ForegroundColor Cyan
Write-Host ""

# Check if running as admin
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "ERROR: This script must be run as Administrator!" -ForegroundColor Red
    Write-Host "Right-click PowerShell and select 'Run as Administrator'" -ForegroundColor Yellow
    exit 1
}

# Check if Chocolatey is installed
if (-not (Get-Command choco -ErrorAction SilentlyContinue)) {
    Write-Host "Installing Chocolatey..." -ForegroundColor Yellow
    Set-ExecutionPolicy Bypass -Scope Process -Force
    [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
    Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))

    # Refresh environment
    Update-PathFromEnvironment
}

Write-Host ""
Write-Host "Installing development tools..." -ForegroundColor Cyan
Write-Host ""

# Install Visual Studio Build Tools
Write-Host "[1/6] Installing Visual Studio 2022 Build Tools..." -ForegroundColor Yellow
choco install visualstudio2022buildtools -y --no-progress
if ($LASTEXITCODE -ne 0) { Write-Host "Warning: VS Build Tools may already be installed" -ForegroundColor Yellow }

Write-Host "[1b/6] Installing C++ build workload..." -ForegroundColor Yellow
choco install visualstudio2022-workload-vctools -y --no-progress
if ($LASTEXITCODE -ne 0) { Write-Host "Warning: C++ workload may already be installed" -ForegroundColor Yellow }

# Install Rust
Write-Host "[2/6] Installing Rust..." -ForegroundColor Yellow
choco install rustup.install -y --no-progress
if ($LASTEXITCODE -ne 0) { Write-Host "Warning: Rust may already be installed" -ForegroundColor Yellow }

# Refresh PATH and ensure stable toolchain is set up
Update-PathFromEnvironment
if (Get-Command rustup -ErrorAction SilentlyContinue) {
    Write-Host "Setting up Rust stable toolchain..." -ForegroundColor Yellow
    rustup default stable
}

# Install Bun
Write-Host "[3/6] Installing Bun..." -ForegroundColor Yellow
choco install bun -y --no-progress
if ($LASTEXITCODE -ne 0) { Write-Host "Warning: Bun may already be installed" -ForegroundColor Yellow }

# Install protoc
Write-Host "[4/6] Installing Protocol Buffers compiler..." -ForegroundColor Yellow
choco install protoc -y --no-progress
if ($LASTEXITCODE -ne 0) { Write-Host "Warning: protoc may already be installed" -ForegroundColor Yellow }

# Install make
Write-Host "[5/6] Installing Make..." -ForegroundColor Yellow
choco install make -y --no-progress
if ($LASTEXITCODE -ne 0) { Write-Host "Warning: make may already be installed" -ForegroundColor Yellow }

# Install OpenSSL with static libraries (required for SQLCipher/libsqlite3-sys)
Write-Host "[6/6] Installing OpenSSL with static libraries..." -ForegroundColor Yellow

# Check if OpenSSL is already installed with static libs
$existingOpenSSL = choco list --local-only openssl --limit-output 2>$null | Select-String "openssl"
$libcryptoExists = Test-Path "C:\Program Files\OpenSSL-Win64\lib\libcrypto.lib"
$vcLibExists = Test-Path "C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD\libcrypto.lib"

$shouldInstallOpenSSL = $true

if ($existingOpenSSL -and -not $libcryptoExists -and -not $vcLibExists) {
    if ($Force) {
        Write-Host "Note: Removing existing OpenSSL to reinstall with static libraries." -ForegroundColor Yellow
        Write-Host "      If you need OpenSSL for other projects, you may need to reinstall it after." -ForegroundColor Yellow
        choco uninstall openssl -y --no-progress 2>$null
    } else {
        Write-Host "Warning: Existing OpenSSL installation lacks static libraries (.lib files)." -ForegroundColor Yellow
        Write-Host "         The build may fail with LNK1181 errors." -ForegroundColor Yellow
        Write-Host "         To reinstall OpenSSL with static libs, run:" -ForegroundColor Yellow
        Write-Host "           .\scripts\setup-windows.ps1 -Force" -ForegroundColor Cyan
        Write-Host "         This will uninstall the current OpenSSL, which may affect other software." -ForegroundColor Yellow
        $shouldInstallOpenSSL = $false
    }
} elseif ($existingOpenSSL -and ($libcryptoExists -or $vcLibExists)) {
    Write-Host "OpenSSL with static libraries already installed, skipping." -ForegroundColor Green
    $shouldInstallOpenSSL = $false
}

if ($shouldInstallOpenSSL) {
    choco install openssl -y --no-progress
    if ($LASTEXITCODE -ne 0) { Write-Host "Warning: OpenSSL installation may have issues" -ForegroundColor Yellow }
}

# Set OPENSSL_DIR environment variable
$opensslPath = "C:\Program Files\OpenSSL-Win64"
if (Test-Path $opensslPath) {
    Write-Host "Setting OPENSSL_DIR environment variable..." -ForegroundColor Yellow
    [System.Environment]::SetEnvironmentVariable("OPENSSL_DIR", $opensslPath, "Machine")
    $env:OPENSSL_DIR = $opensslPath

    # Verify libcrypto.lib exists (required for static linking)
    $libcryptoPath = Join-Path $opensslPath "lib\libcrypto.lib"
    if (Test-Path $libcryptoPath) {
        Write-Host "Verified: libcrypto.lib found at $libcryptoPath" -ForegroundColor Green
    } else {
        # Check for VC folder structure (some versions use lib\VC\x64\MD)
        $vcLibPath = Join-Path $opensslPath "lib\VC\x64\MD"
        if (Test-Path (Join-Path $vcLibPath "libcrypto.lib")) {
            Write-Host "Found libs in $vcLibPath, copying to lib\ for linker compatibility..." -ForegroundColor Yellow
            Copy-Item "$vcLibPath\*.lib" (Join-Path $opensslPath "lib\") -Force
            Write-Host "Copied OpenSSL libs to $opensslPath\lib\" -ForegroundColor Green
        } else {
            Write-Host "Warning: libcrypto.lib not found. Build may fail with LNK1181 error." -ForegroundColor Yellow
            Write-Host "See HOW_TO_BUILD_BAGZ_ON_WINDOWS.md troubleshooting section." -ForegroundColor Yellow
        }
    }
} else {
    # Try alternate location
    $opensslPath = "C:\Program Files\OpenSSL"
    if (Test-Path $opensslPath) {
        [System.Environment]::SetEnvironmentVariable("OPENSSL_DIR", $opensslPath, "Machine")
        $env:OPENSSL_DIR = $opensslPath
    } else {
        Write-Host "Warning: OpenSSL installed but path not found. You may need to set OPENSSL_DIR manually." -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "=== Installation Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "IMPORTANT: Close this terminal and open a new one for PATH changes to take effect!" -ForegroundColor Yellow
Write-Host ""
Write-Host "Then run these commands to build:" -ForegroundColor Cyan
Write-Host "  cd <your-bagz-clone-directory>  # Navigate to where you cloned bagz" -ForegroundColor White
Write-Host "  make install" -ForegroundColor White
Write-Host "  make tauri-build" -ForegroundColor White
Write-Host ""
Write-Host "The installer will be at:" -ForegroundColor Cyan
Write-Host "  target\release\bundle\nsis\bagZ_*_x64-setup.exe" -ForegroundColor White
