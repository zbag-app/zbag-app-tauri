# Building zbag on Linux

This guide walks you through setting up your Linux system to build zbag. No programming experience required.

---

## Before You Start

You will need:
- A computer running Ubuntu 22.04 or newer (or a compatible Debian-based distribution)
- A GitHub account
- About 20 GB of free disk space
- A stable internet connection

---

## Step 1: Open a Terminal

For all the following steps, you will use the **Terminal** - a text-based way to run commands.

### On Ubuntu:
- Press **Ctrl + Alt + T** to open Terminal
- Or click the **Activities** button, type **Terminal**, and click the icon

---

## Step 2: Update Your System

Before installing anything, update your package lists to ensure you get the latest versions.

Copy and paste this command into Terminal and press Enter:

```bash
sudo apt update && sudo apt upgrade -y
```

You will be asked for your password. Type it (it will not show as you type) and press Enter.

---

## Step 3: Install Required System Packages

zbag needs several system libraries and tools to build. Install them all with this command:

```bash
sudo apt install -y build-essential curl wget git pkg-config libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev protobuf-compiler
```

This installs:
- **build-essential** - Compilers (gcc, g++) and make
- **curl, wget** - Tools to download files
- **git** - Version control
- **pkg-config** - Library configuration helper
- **libssl-dev** - SSL/TLS support
- **libgtk-3-dev** - GTK toolkit for the UI
- **libwebkit2gtk-4.1-dev** - WebKit for Tauri webview
- **libayatana-appindicator3-dev** - System tray support
- **librsvg2-dev** - SVG rendering
- **protobuf-compiler** - Protocol Buffers compiler (required for Zcash libraries)

---

## Step 4: Install GitHub CLI

The GitHub CLI makes it easy to authenticate and clone repositories.

```bash
sudo apt install -y gh
```

If gh is not available in your distribution, install it manually:

```bash
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg
sudo chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null
sudo apt update
sudo apt install -y gh
```

---

## Step 5: Install Rust

Rust is the programming language zbag is written in.

Copy and paste this command into Terminal:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

When prompted with installation options, just press **Enter** to accept the default (option 1).

After it finishes, run this command to activate Rust:

```bash
source "$HOME/.cargo/env"
```

### Verify Rust Works

```bash
rustc --version
```

You should see something like `rustc 1.xx.x`. If you see "command not found", close and reopen Terminal, then try again.

---

## Step 6: Install Bun

Bun is a tool for building the user interface.

Copy and paste this command into Terminal:

```bash
curl -fsSL https://bun.sh/install | bash
```

After it finishes, **close Terminal completely** and **reopen it**.

### Verify Bun Works

```bash
bun --version
```

You should see a version number like `1.x.x`.

---

## Step 7: Set Up GitHub Access

We need to connect your computer to GitHub to download the code.

Run this command:

```bash
gh auth login
```

You will be asked several questions. Choose these options:
1. **Where do you use GitHub?** - Select `GitHub.com`
2. **Preferred protocol** - Select `HTTPS`
3. **Authenticate Git** - Select `Yes`
4. **How to authenticate** - Select `Login with a web browser`

A code will be displayed. Press Enter, and your web browser will open. Paste the code into the GitHub website and authorize the app.

---

## Step 8: Download and Build zbag

Now we download the code and build the app.

Run these commands one at a time:

```bash
gh repo clone <repository-url> zbag-app-tauri
cd zbag-app-tauri
```

```bash
make install
```

```bash
make tauri-build
```

The last command (`make tauri-build`) will take several minutes. You will see a lot of text scrolling by - this is normal.

### Success

When the build completes without errors, your app is ready.

The built application bundles are located at:

```
apps/zbag-app-tauri/src-tauri/target/release/bundle/
```

Inside you will find:
- **deb/** - Debian package (.deb) for Ubuntu/Debian
- **appimage/** - AppImage portable executable
- **rpm/** - RPM package for Fedora/RHEL (if applicable)

You can open this folder in your file manager by running:

```bash
xdg-open apps/zbag-app-tauri/src-tauri/target/release/bundle/
```

### Installing the Debian Package

To install the .deb package:

```bash
sudo dpkg -i apps/zbag-app-tauri/src-tauri/target/release/bundle/deb/*.deb
```

### Running the AppImage

To run the AppImage directly:

```bash
chmod +x apps/zbag-app-tauri/src-tauri/target/release/bundle/appimage/*.AppImage
./apps/zbag-app-tauri/src-tauri/target/release/bundle/appimage/*.AppImage
```

---

## Troubleshooting

### "command not found" errors

Close Terminal completely and reopen it. Try the command again.

### "protoc: command not found" or gRPC errors

Run:

```bash
sudo apt install -y protobuf-compiler
```

### Build fails with Rust errors

Run:

```bash
rustup update
```

Then try `make tauri-build` again.

### Missing WebKit or GTK libraries

If you see errors about missing webkit2gtk or gtk, ensure you installed the correct version:

```bash
sudo apt install -y libwebkit2gtk-4.1-dev libgtk-3-dev
```

Note: Ubuntu 22.04 and newer use webkit2gtk-4.1. Older distributions may need webkit2gtk-4.0.

### Password prompt does not accept input

When Terminal asks for your password, the characters do not appear as you type. This is normal - just type your password and press Enter.

### Fedora / RHEL / Other Distributions

For Fedora, use dnf instead of apt:

```bash
sudo dnf install -y gcc g++ make curl wget git pkg-config openssl-devel gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel protobuf-compiler
```

For Arch Linux, use pacman:

```bash
sudo pacman -S --needed base-devel curl wget git pkg-config openssl gtk3 webkit2gtk-4.1 libappindicator-gtk3 librsvg protobuf
```

---

## Getting Help

If you get stuck, take a screenshot of the error message and share it with the team.
