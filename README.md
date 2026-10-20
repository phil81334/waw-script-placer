# WaW Script Placer

**An unofficial community app. Not affiliated with or endorsed by the UGX team.**

A starter mod/map creator for World at War, made by [phil81334](https://github.com/phil81334), a community member, for the community. Built with JavaScript, Vite and Tauri.

![WaW Script Placer creating a map from a custom map source](images/waw-script-placer.png)

![Delete tab with separate selections for starter files, other game files and AppData](images/waw-script-placer-delete-tab.png)

More screenshots: [Expanded file lists](images/waw-script-placer-delete-tab-fully-expanded.png) · [Deletion review](images/waw-script-placer-delete-tab-review-deletion-popup.png) · [Deletion complete](images/waw-script-placer-delete-tab-successful-deletion-output.png).

## How does this differ from UGX Script Placer 2.0.1?

**General generation is 1:1: the same 18 starter files, the same contents and the same map-name/display-name substitutions.** Blank, Setup and Custom work from the same starter resources. This is not a modified starter pack.

The UI is new, and overwrite handling is fixed: **Skip** leaves the existing file untouched, **Replace** replaces it, and **Back Up + Replace** keeps a backup first.

**Blue = included. Red = left out.** No FX options, Menu options, UGX-MOD installation, patch installer or registry settings.

![UGX Script Placer: General and Generate outlined in blue; excluded features tinted red](images/ugx-script-placer.png)

Checked against actual UGX-generated Blank, Setup and Custom outputs: all 54 files matched after accounting for the different map names.

## Use

1. Download the executable from [Releases](https://github.com/phil81334/waw-script-placer/releases).
2. Put it beside `CoDWaW.exe`, after installing your Modtools and UGX patch.
3. Open it, enter a map name, choose a map source, and click **Generate**.

The app always uses its own folder. Windows needs [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) (normally already installed).

## Delete a map

Open **Delete**, choose a map and click **Scan files**. Select individual files or whole sections: **Starter files**, **Other files in the game folder**, and **Playable copy in AppData**. Review the list, then confirm.

Deletion is permanent, including any edits in selected files. The game-folder section includes everything else inside that mod’s folder and recognised map-specific build outputs and backups in shared folders. Unlisted files are left alone.

## Run from source

1. Install [Node.js LTS](https://nodejs.org/) and the [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/#windows): Rust and Microsoft C++ Build Tools.
2. Download and extract this repository, or clone it. Open its folder in VS Code.
3. In **Terminal → New Terminal**, run:

   ```sh
   npm install
   npm run dev
   ```

After the first setup, click the play button beside **dev** in VS Code’s **NPM Scripts** panel. The first Rust build takes longer.

Development writes to `.local/test-root`, not your game. Set `WAW_ROOT` before running if you want another test folder.

## Commands

| Command | What it does |
| --- | --- |
| `npm run dev` | Opens the desktop app for development. |
| `npm run dev:web` | Starts a browser preview. It cannot generate files. |
| `npm run build` | Creates the desktop EXE at `release/waw-script-placer.exe`. |
| `npm run build:web` | Builds only the UI files in `dist`. No EXE. The desktop build runs this automatically. |

Starter templates come from [UGX Script Placer 2.0.1](https://www.ugx-mods.com/forum/applications-and-tools/36/app-ugx-script-placer-2-0-1-beta-now-with-ugx-mod-compatibility/4221/). See [ATTRIBUTION.md](ATTRIBUTION.md) for credits.

Original app code: [GPL-3.0 licence](LICENSE). Starter templates retain their existing terms.

