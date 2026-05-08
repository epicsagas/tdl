# tdl installer script for Windows
# Usage: irm https://raw.githubusercontent.com/epicsagas/tdl/main/scripts/install.ps1 | iex

$ErrorActionPreference = "Stop"

# Detect architecture
if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
    $ARCH = "aarch64"
} else {
    $ARCH = "x86_64"
}

$PLATFORM = "windows"
$BINARY_NAME = "tdl-$ARCH-$PLATFORM.exe"

# Get latest release version
try {
    $LATEST_URL = "https://api.github.com/repos/epicsagas/tdl/releases/latest"
    $VERSION = (Invoke-RestMethod -Uri $LATEST_URL).tag_name
} catch {
    Write-Host "Failed to fetch latest version, using main branch" -ForegroundColor Yellow
    $VERSION = "main"
}

Write-Host "Installing tdl $VERSION for Windows-$ARCH" -ForegroundColor Green

# Download URL
$DOWNLOAD_URL = "https://github.com/epicsagas/tdl/releases/download/$VERSION/$BINARY_NAME"

# Install directory
$INSTALL_DIR = Join-Path $env:USERPROFILE ".local\bin"
New-Item -ItemType Directory -Force -Path $INSTALL_DIR | Out-Null

$OUTPUT_PATH = Join-Path $INSTALL_DIR "tdl.exe"

Write-Host "Downloading from $DOWNLOAD_URL"
try {
    Invoke-WebRequest -Uri $DOWNLOAD_URL -OutFile $OUTPUT_PATH -UseBasicParsing
} catch {
    Write-Host "Download failed" -ForegroundColor Red
    Write-Host "Try downloading manually from: https://github.com/epicsagas/tdl/releases"
    exit 1
}

Write-Host "✓ Installed to $OUTPUT_PATH" -ForegroundColor Green

# Check PATH
$PATH_DIRS = $env:PATH -split ';'
if ($INSTALL_DIR -notin $PATH_DIRS) {
    Write-Host ""
    Write-Host "⚠ $INSTALL_DIR is not in your PATH" -ForegroundColor Yellow
    Write-Host "Add it manually:"
    Write-Host "  [Environment]::SetEnvironmentVariable('Path', `"`${env:PATH};$INSTALL_DIR`", 'User')"
}

Write-Host ""
Write-Host "Run: tdl --version"
