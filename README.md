# Steam Downloader & AutoCrack

A modern desktop application to download clean Steam files using Morrenus API and DepotDownloaderMod.

## Prerequisites
- Node.js (v16+)
- Rust (Cargo)
- .NET 9.0 Runtime (The app checks for this)

## Setup
1. Open a terminal in this directory.
2. Install frontend dependencies:
   ```bash
   npm install
   ```
3. Run the application in development mode:
   ```bash
   npm run tauri dev
   ```

## Features
- Check for .NET 9.0 installation.
- Download Clean Files via Morrenus Manifest Manager.
- Auto-extract and run DepotDownloaderMod.

## Architecture
- **Frontend**: React + Vite (Dark Mode, Premium Design)
- **Backend**: Rust (Tauri) for robust API handling and process execution.
