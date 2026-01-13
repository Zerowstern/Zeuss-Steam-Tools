$ErrorActionPreference = "Stop"
$ProjectPath = "c:\Programs\steam-download-and-autocrack"
$BuildPath = "$ProjectPath\src-tauri\target\release"
$DistDir = "$ProjectPath\SAGR_STEAM_TOOLS_Dist"
$ZipName = "$ProjectPath\SAGR_STEAM_TOOLS_Release.zip"

Write-Host "Starting Bundling Process..."

# 1. Clean up previous dist
if (Test-Path $DistDir) {
    Remove-Item -Recurse -Force $DistDir
}
if (Test-Path $ZipName) {
    Remove-Item -Force $ZipName
}

# 2. Create Dist Directory
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

# 3. Copy Executable
$ExeSource = "$BuildPath\sagr-steam-tools.exe"
if (-not (Test-Path $ExeSource)) {
    Write-Error "Executable not found at $ExeSource. Did you build the project?"
}
Copy-Item -Path $ExeSource -Destination $DistDir

# 4. Copy Dependencies
$Dependencies = @(
    "DepotDownloaderMod",
    "emu-win-release",
    "Steamless.v3.1.0.5.-.by.atom0s"
)

foreach ($Dep in $Dependencies) {
    $Source = "$ProjectPath\$Dep"
    if (Test-Path $Source) {
        Copy-Item -Path $Source -Destination "$DistDir\$Dep" -Recurse
    } else {
        Write-Warning "Dependency folder not found: $Source"
    }
}

# 5. Zip it up
Write-Host "Compressing to $ZipName..."
Compress-Archive -Path "$DistDir\*" -DestinationPath $ZipName

Write-Host "Bundling Complete!"
Write-Host "Navigate to $ProjectPath to find 'SAGR_STEAM_TOOLS_Release.zip'"
