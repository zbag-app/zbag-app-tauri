# Building zbag on Windows

This guide walks you through setting up your Windows system to build zbag. No programming experience required.

---

## Table of Contents

- [Before You Start](#before-you-start)
- [Step 1: Open PowerShell as Administrator](#step-1-open-powershell-as-administrator)
- [Step 2: Allow Script Execution](#step-2-allow-script-execution)
- [Step 3: Install Chocolatey](#step-3-install-chocolatey-if-not-installed)
- [Step 4: Install GitHub CLI](#step-4-install-github-cli)
- [Step 5: Set Up GitHub Access](#step-5-set-up-github-access)
- [Step 6: Download the Code](#step-6-download-the-code)
- [Step 7: Run the Setup Script](#step-7-run-the-setup-script)
- [Step 8: Open a New Terminal](#step-8-open-a-new-terminal-critical)
- [Step 9: Build zbag](#step-9-build-zbag)
- [Troubleshooting](#troubleshooting)
- [Getting Help](#getting-help)

---

## Before You Start

### Requirements

| Requirement | Minimum |
|-------------|---------|
| Operating System | Windows 10 (build 1903+) or Windows 11 |
| RAM | 8 GB (16 GB recommended) |
| Disk Space | 20 GB free (see breakdown below) |
| Internet | Stable broadband connection |

You will also need a GitHub account to download the source code.

### Disk Space Breakdown

| Component | Approximate Size |
|-----------|------------------|
| Visual Studio Build Tools | ~6 GB |
| Rust toolchain | ~2 GB |
| Project dependencies | ~3 GB |
| Build artifacts | ~5 GB |
| Other tools (Bun, protoc, etc.) | ~1 GB |

---

## Step 1: Open PowerShell as Administrator

All the following steps require Administrator privileges.

### How to open PowerShell as Administrator:
1. Press **Windows + X** to open the Power User menu
2. Click **Terminal (Admin)** or **Windows PowerShell (Admin)**
3. If prompted by User Account Control, click **Yes**

You should see a window with "Administrator" in the title bar.

---

## Step 2: Allow Script Execution

Windows restricts running scripts by default. Run this command to allow scripts for this session:

```powershell
Set-ExecutionPolicy -ExecutionPolicy Bypass -Scope Process
```

When prompted, type **Y** and press Enter.

> **Security Note:** The `-Scope Process` flag limits this change to the current PowerShell window only. Once you close this window, the execution policy reverts to its previous setting. This is the safest way to temporarily allow scripts without permanently changing your system's security settings.

---

## Step 3: Install Chocolatey (if not installed)

Chocolatey is a package manager for Windows that makes installing tools easy.

Check if Chocolatey is already installed:

```powershell
choco --version
```

If you see a version number, skip to Step 4. If you see an error, install Chocolatey:

```powershell
Set-ExecutionPolicy Bypass -Scope Process -Force
[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
```

> **Security Note:** This installation command downloads and executes a script from Chocolatey's official website. This is the standard installation method recommended by Chocolatey. If you prefer to verify the script before running it, you can:
> 1. Download the script manually: visit https://community.chocolatey.org/install.ps1
> 2. Review the script contents
> 3. Save it locally and run it with: `.\install.ps1`
>
> For more information, see [Chocolatey's installation documentation](https://chocolatey.org/install).

---

## Step 4: Install GitHub CLI

The GitHub CLI makes it easy to authenticate and clone repositories.

```powershell
choco install gh -y
```

---

## Step 5: Set Up GitHub Access

We need to connect your computer to GitHub to download the code.

**Close your current PowerShell window and open a new Administrator PowerShell** (to pick up the new gh command).

Then run:

```powershell
gh auth login
```

You will be asked several questions. Choose these options:
1. **Where do you use GitHub?** - Select `GitHub.com`
2. **Preferred protocol** - Select `HTTPS`
3. **Authenticate Git** - Select `Yes`
4. **How to authenticate** - Select `Login with a web browser`

A code will be displayed. Press Enter, and your web browser will open. Paste the code into the GitHub website and authorize the app.

---

## Step 6: Download the Code

Now download the zbag source code. First, navigate to your home directory, then clone the repository:

```powershell
cd $env:USERPROFILE
gh repo clone <repository-url> zbag-app-tauri
cd zbag-app-tauri
```

This creates the folder `%USERPROFILE%\zbag-app-tauri` (typically `C:\Users\YourName\zbag-app-tauri`).

> **Note:** If you prefer a different location, clone there instead and remember the path for Step 8.

---

## Step 7: Run the Setup Script

The setup script installs all required development tools:
- Visual Studio 2022 Build Tools with C++ workload
- Rust (via rustup)
- Bun (JavaScript runtime)
- Protocol Buffers compiler (protoc)
- Make (build automation)
- OpenSSL (required for SQLCipher)

Run the setup script:

```powershell
Set-ExecutionPolicy -ExecutionPolicy Bypass -Scope Process
.\scripts\setup-windows.ps1
```

This will take several minutes as it downloads and installs each tool.

---

## Step 8: Open a New Terminal (Critical!)

**IMPORTANT:** After the setup script completes, you MUST close your current terminal and open a new one. This is required for the PATH changes to take effect.

1. Close the current PowerShell window completely
2. Open a new PowerShell window (Administrator is no longer required)
3. Navigate back to the zbag folder:

```powershell
cd $env:USERPROFILE\zbag
```

> **Note:** This assumes you followed Step 6 and cloned to your home directory. If you cloned elsewhere, navigate to that location instead.

---

## Step 9: Build zbag

Now build the application:

**Option A: Using the build script (recommended)**

```powershell
.\scripts\build-tauri.ps1
```

This script handles all build steps automatically, including installing dependencies and configuring OpenSSL.

**Option B: Using make directly**

```powershell
make install
```

```powershell
make tauri-build
```

The build will take several minutes. You will see a lot of text scrolling by - this is normal.

### Success

When the build completes without errors, your app is ready.

The built installer is located at:

```
target\release\bundle\nsis\
```

Inside you will find an NSIS installer (.exe) that you can run to install zbag on your system.

You can open this folder in File Explorer by running:

```powershell
explorer target\release\bundle\nsis
```

---

## Troubleshooting

### "cannot be loaded because running scripts is disabled"

Run this command before running the setup script:

```powershell
Set-ExecutionPolicy -ExecutionPolicy Bypass -Scope Process
```

### "cargo: command not found" or "rustc: command not found"

This means PATH changes haven't taken effect. **Close your terminal completely and open a new one.** This is the most common issue.

### "OPENSSL_DIR is not set" or OpenSSL errors

The setup script should set this automatically. If you still see errors, set it manually:

```powershell
[System.Environment]::SetEnvironmentVariable("OPENSSL_DIR", "C:\Program Files\OpenSSL-Win64", "Machine")
```

Then close and reopen your terminal.

If OpenSSL is installed elsewhere, find it with:

```powershell
Get-ChildItem "C:\Program Files" -Directory | Where-Object { $_.Name -like "*OpenSSL*" }
```

### "LNK1181: cannot open input file 'libcrypto.lib'"

This error means OpenSSL is installed but the static library files (`.lib`) are in a nested folder where the linker can't find them. The Chocolatey OpenSSL package places libs in `lib\VC\x64\MD\` instead of `lib\`.

**Solution 1: Copy libs to the expected location (recommended)**

Run in Administrator PowerShell:

```powershell
Copy-Item "C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD\*.lib" "C:\Program Files\OpenSSL-Win64\lib\" -Force
```

Then rebuild with `make tauri-build`.

**Solution 2: Set OPENSSL_LIB_DIR environment variable**

```powershell
[System.Environment]::SetEnvironmentVariable("OPENSSL_LIB_DIR", "C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD", "Machine")
```

Then open a new terminal and do a clean rebuild:

```powershell
cargo clean
make tauri-build
```

**Solution 3: Use vcpkg (alternative)**

vcpkg provides a more predictable directory structure:

```powershell
# Install vcpkg
git clone https://github.com/microsoft/vcpkg.git $env:USERPROFILE\vcpkg
& "$env:USERPROFILE\vcpkg\bootstrap-vcpkg.bat"

# Install OpenSSL (x64, static)
& "$env:USERPROFILE\vcpkg\vcpkg" install openssl:x64-windows-static

# Set environment variable
[System.Environment]::SetEnvironmentVariable("OPENSSL_DIR", "$env:USERPROFILE\vcpkg\installed\x64-windows-static", "Machine")
```

Then open a new terminal and rebuild.

### "protoc: command not found" or protobuf errors

Ensure protoc was installed:

```powershell
choco install protoc -y
```

Then close and reopen your terminal.

### Build fails with Rust errors

Update Rust to the latest version:

```powershell
rustup update
```

Then try `make tauri-build` again.

### Visual Studio Build Tools errors

If you see errors about missing MSVC or link.exe, reinstall the C++ workload:

```powershell
choco install visualstudio2022-workload-vctools -y --force
```

Then close and reopen your terminal.

### "make: command not found"

Ensure make was installed:

```powershell
choco install make -y
```

Then close and reopen your terminal.

### Build is extremely slow

The first build compiles many dependencies and can take 10+ minutes. Subsequent builds will be much faster.

Ensure you have at least 8 GB of RAM available. Close other applications if needed.

---

## Getting Help

### Report Issues

If you encounter problems not covered in the troubleshooting section, contact the maintainer:

- **Security and licensing contact:** `security@zbag.app`

When reporting an issue, include:
1. The exact error message (copy/paste, not a screenshot if possible)
2. Which step you were on when the error occurred
3. Your Windows version (`winver` command)
4. Output of `rustc --version` and `cargo --version` (if Rust is installed)

### Capturing Verbose Output

To capture detailed build output for troubleshooting:

```powershell
# Redirect all output to a file
make tauri-build 2>&1 | Tee-Object -FilePath build-log.txt
```

This creates `build-log.txt` in your zbag folder with the complete build output.

### Common Log Locations

| Log Type | Location |
|----------|----------|
| Cargo build cache | `target\` folder in project root |
| Rust toolchain logs | `%USERPROFILE%\.rustup\` |
| Chocolatey logs | `C:\ProgramData\chocolatey\logs\` |
| npm/bun cache | `%USERPROFILE%\.bun\` |

### Additional Resources

- [Rust Installation Guide](https://www.rust-lang.org/tools/install)
- [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
