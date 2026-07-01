# Build script for zbag on Windows
# Sets OpenSSL environment variables and runs the Tauri build

$ErrorActionPreference = "Stop"

# Track whether we changed directory (for safe cleanup in finally block)
$pushedLocation = $false

# Determine project root from script location
$projectRoot = if ($PSScriptRoot) {
    Split-Path $PSScriptRoot -Parent
} else {
    # Fallback: assume current directory is project root
    Get-Location
}

# Change to project root first, so all checks happen in the correct context
Push-Location $projectRoot
$pushedLocation = $true

try {
    # Check for required tools
    if (-not (Get-Command make -ErrorAction SilentlyContinue)) {
        Write-Host "ERROR: 'make' is not found in PATH." -ForegroundColor Red
        Write-Host "Install make via chocolatey: choco install make" -ForegroundColor Yellow
        exit 1
    }

    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Host "ERROR: 'cargo' is not found in PATH." -ForegroundColor Red
        Write-Host "Run scripts\setup-windows.ps1 as Administrator to install Rust." -ForegroundColor Yellow
        exit 1
    }

    if (-not (Get-Command bun -ErrorAction SilentlyContinue)) {
        Write-Host "ERROR: 'bun' is not found in PATH." -ForegroundColor Red
        Write-Host "Run scripts\setup-windows.ps1 as Administrator to install Bun." -ForegroundColor Yellow
        exit 1
    }

    if (-not (Get-Command protoc -ErrorAction SilentlyContinue)) {
        Write-Host "ERROR: 'protoc' is not found in PATH." -ForegroundColor Red
        Write-Host "Run scripts\setup-windows.ps1 as Administrator to install protoc." -ForegroundColor Yellow
        exit 1
    }

    # Verify we're in the correct directory
    if (-not (Test-Path "Makefile")) {
        Write-Host "ERROR: Makefile not found. Run this script from the zbag repository root." -ForegroundColor Red
        exit 1
    }

    # Set OpenSSL environment variables (use existing if set, derive from OPENSSL_DIR if possible)
    if (-not $env:OPENSSL_DIR) {
        $opensslPath = "C:\Program Files\OpenSSL-Win64"
        if (Test-Path $opensslPath) {
            $env:OPENSSL_DIR = $opensslPath
        }
    }
    if (-not $env:OPENSSL_LIB_DIR -and $env:OPENSSL_DIR) {
        $vcLibPath = "$env:OPENSSL_DIR\lib\VC\x64\MD"
        if (Test-Path "$vcLibPath\libcrypto.lib") {
            $env:OPENSSL_LIB_DIR = $vcLibPath
        } else {
            $env:OPENSSL_LIB_DIR = "$env:OPENSSL_DIR\lib"
        }
    }
    if (-not $env:OPENSSL_INCLUDE_DIR -and $env:OPENSSL_DIR) {
        $env:OPENSSL_INCLUDE_DIR = "$env:OPENSSL_DIR\include"
    }

    Write-Host "=== zbag Windows Build ===" -ForegroundColor Cyan
    Write-Host "OPENSSL_DIR: $env:OPENSSL_DIR"
    Write-Host "OPENSSL_LIB_DIR: $env:OPENSSL_LIB_DIR"
    Write-Host ""

    # Check if OPENSSL_LIB_DIR is set
    if (-not $env:OPENSSL_LIB_DIR) {
        Write-Host "ERROR: OPENSSL_LIB_DIR is not set and OpenSSL was not found at the default location." -ForegroundColor Red
        Write-Host "Run scripts\setup-windows.ps1 as Administrator to install OpenSSL." -ForegroundColor Yellow
        exit 1
    }

    # Verify libcrypto.lib exists
    $libcryptoPath = Join-Path $env:OPENSSL_LIB_DIR "libcrypto.lib"
    if (-not (Test-Path $libcryptoPath)) {
        Write-Host "ERROR: libcrypto.lib not found at $libcryptoPath" -ForegroundColor Red
        Write-Host "Run scripts\setup-windows.ps1 as Administrator to fix this." -ForegroundColor Yellow
        exit 1
    }

    Write-Host "Found libcrypto.lib" -ForegroundColor Green
    Write-Host ""

    # Install dependencies
    Write-Host "Installing dependencies..." -ForegroundColor Cyan
    & make install

    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "=== Dependency Installation Failed ===" -ForegroundColor Red
        exit $LASTEXITCODE
    }

    Write-Host ""

    # Run the build
    Write-Host "Starting build..." -ForegroundColor Cyan
    & make tauri-build

    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "=== Build Successful ===" -ForegroundColor Green

        # Find actual build outputs
        $exePath = "target\release\zbag-app-tauri.exe"
        if (Test-Path $exePath) {
            Write-Host "Executable: $exePath"
        }

        $nsisDir = "target\release\bundle\nsis"
        if (Test-Path $nsisDir) {
            $installers = Get-ChildItem -Path $nsisDir -Filter "*-setup.exe" -ErrorAction SilentlyContinue
            if ($installers) {
                foreach ($installer in $installers) {
                    Write-Host "Installer:  $($installer.FullName)"
                }
            }
        }
    } else {
        Write-Host ""
        Write-Host "=== Build Failed ===" -ForegroundColor Red
        exit $LASTEXITCODE
    }
} finally {
    if ($pushedLocation) {
        Pop-Location
    }
}
