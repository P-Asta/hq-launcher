<div align="center">

![HQ Launcher](https://raw.githubusercontent.com/p-asta/hq-launcher/main/assets/banner.png)

</div>

[![GitHub Release](https://img.shields.io/github/v/release/p-asta/hq-launcher)](https://github.com/P-Asta/hq-launcher/releases)
[![Discord](https://img.shields.io/discord/1255306516672806972?style=flat&label=discord)](https://discord.com/invite/usYCEz49Je)

A game launcher built with [React](https://react.dev/) and [Tauri](https://tauri.app/), designed to make HQ(High Quota) challenge configuration faster and more convenient.

## Features

- **Steam Authentication**: Secure login with Steam account
- **Version Management**: Install and select different game versions
- **Game Launch/Stop**: One-click game execution and termination
- **Mod Management**: Search, enable/disable, and configure mods
- **Config Editor**: Built-in BepInEx configuration file editor
- **Auto Updates**: Automatic updates for both launcher and game versions
- **Practice Mode**: This is a mode that runs by adding a practice plugin.


## Practice Mode
- [LethalDevMode](https://thunderstore.io/c/lethal-company/p/megumin/LethalDevMode/)(v45+)
- [Imperium](https://thunderstore.io/c/lethal-company/p/giosuel/Imperium/)(v50+)
- [CruiserJumpPractice](https://thunderstore.io/c/lethal-company/p/aoirint/CruiserJumpPractice/)(v56+)

## Installation

### Windows


**From Github**

- Go to [Releases](https://github.com/p-asta/hq-launcher/releases).
- Download the installer for your desired version (the latest is recommended).
- Run the downloaded file.

> [!NOTE]
> You might get a prompt saying "Windows has protected your PC". In this case, click `More Info` and `Run Anyway`.

> [!TIP]
> If you're unsure about the safety of this app, I would suggest running it through a service like [VirusTotal](https://www.virustotal.com).

### Linux

AppImages and other package formats are available in [Releases](https://github.com/p-asta/hq-launcher/releases).

Want to build it yourself? See the [Development](#development) section below.

## Screenshots

_Main Interface_

![screenshot](https://raw.githubusercontent.com/p-asta/hq-launcher/main/assets/ss/1.png)

## contributer
![contributors](https://contrib.rocks/image?repo=p-asta/hq-launcher)

## Development

### Prerequisites

- Node.js and yarn
- Rust (for Tauri)

### Setup

```bash
# Install dependencies
yarn install

# Run development server
yarn dev

# Run Tauri in development mode
yarn tauri dev

# Build for production
yarn tauri build
```

## Tech Stack

- **Frontend**: React 19, Tailwind CSS
- **Backend**: Tauri 2 (Rust)
- **UI Components**: Radix UI, Lucide React
- **Build Tool**: Vite

## Credits

Built with [Tauri](https://tauri.app/) and [React](https://react.dev/).
