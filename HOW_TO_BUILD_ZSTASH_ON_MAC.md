# Building zSTASH on macOS

This guide walks you through setting up your Mac to build zSTASH. No programming experience required.

---

## Before You Start

You will need:
- A Mac running macOS 14 or newer
- An Apple ID (for Xcode)
- A GitHub account
- About 20 GB of free disk space
- A stable internet connection

---

## Step 1: Install Xcode

Xcode is Apple's developer toolkit. We need it for the compilers that build the app.

1. Open the **App Store** on your Mac
2. Search for **Xcode**
3. Click **Get** and then **Install**
4. Wait for the download to complete (this is large, around 12 GB)
5. Once installed, **open Xcode once** from your Applications folder
6. Accept the license agreement when prompted
7. You can close Xcode after it finishes setting up

### Open Terminal

For the remaining steps, you will use **Terminal** - a text-based way to run commands.

1. Press **Cmd + Space** to open Spotlight
2. Type **Terminal** and press Enter
3. A window with a text prompt will appear

### Install Command Line Tools

Copy and paste this command into Terminal, then press Enter:

```bash
xcode-select --install
```

A popup will appear. Click **Install** and wait for it to complete.

Then run this command (you will need to enter your Mac password):

```bash
sudo xcodebuild -license accept
```

Type your password (it will not show as you type) and press Enter.

---

## Step 2: Install Homebrew

Homebrew is a tool that makes it easy to install developer software on Mac.

Copy and paste this entire command into Terminal and press Enter:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Follow the prompts:
- Press Enter when asked to continue
- Enter your Mac password if prompted

**Important:** When it finishes, Homebrew will display a message that says "Next steps" with commands to run. You must run those commands. They will look something like:

```bash
echo >> /Users/YOURNAME/.zprofile
echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> /Users/YOURNAME/.zprofile
eval "$(/opt/homebrew/bin/brew shellenv)"
```

Copy and run each line it shows you.

### Verify Homebrew Works

Close Terminal completely (Cmd + Q), then reopen it.

Type this command and press Enter:

```bash
brew --version
```

You should see a version number like `Homebrew 4.x.x`. If you see "command not found", go back and run the "Next steps" commands from the Homebrew installation.

---

## Step 3: Install Required Tools via Homebrew

Now we install the tools needed to build zSTASH.

Copy and paste this command into Terminal:

```bash
brew install protobuf pkg-config git gh
```

Wait for it to finish. You will see progress messages as each tool installs.

---

## Step 4: Install Rust

Rust is the programming language zSTASH is written in.

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

## Step 5: Install Bun

Bun is a tool for building the user interface.

Copy and paste this command into Terminal:

```bash
curl -fsSL https://bun.sh/install | bash
```

After it finishes, **close Terminal completely** (Cmd + Q) and **reopen it**.

### Verify Bun Works

```bash
bun --version
```

You should see a version number like `1.x.x`.

---

## Step 6: Set Up GitHub Access

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

## Step 7: Download and Build zSTASH

Now we download the code and build the app.

Run these commands one at a time:

```bash
gh repo clone zstashapp/zstash
```

```bash
cd zstash
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

The built application is located at:

```
apps/zstash-app-tauri/src-tauri/target/release/bundle/macos/zSTASH.app
```

You can open this folder in Finder by running:

```bash
open apps/zstash-app-tauri/src-tauri/target/release/bundle/macos/
```

Drag **zSTASH.app** to your Applications folder to install it.

---

## Troubleshooting

### "command not found" errors

Close Terminal completely (Cmd + Q) and reopen it. Try the command again.

### "protoc: command not found" or gRPC errors

Run:

```bash
brew install protobuf
```

### Build fails with Rust errors

Run:

```bash
rustup update
```

Then try `make tauri-build` again.

### "xcodebuild requires Xcode" error

Open Xcode from your Applications folder, accept any prompts, then try again.

### Password prompt does not accept input

When Terminal asks for your password, the characters do not appear as you type. This is normal - just type your password and press Enter.

### Homebrew "Next steps" were skipped

Run this command:

```bash
eval "$(/opt/homebrew/bin/brew shellenv)"
```

Then add it permanently by running:

```bash
echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
```

---

## Getting Help

If you get stuck, take a screenshot of the error message and share it with the team.
