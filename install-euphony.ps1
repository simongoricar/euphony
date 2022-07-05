$ErrorActionPreference = "Stop"

function Add-Directory {
    param (
        [Parameter(Mandatory=$true)]
        $DirectoryPath
    )

    if (-not (Test-Path $DirectoryPath -Type Container)) {
        Write-Host -ForegroundColor DarkGray "Creating directory: $DirectoryPath"
        New-Item -Path $DirectoryPath -ItemType Directory | Out-Null
    }
}

Write-Host -ForegroundColor Yellow "This script will compile euphony and install it into the ./bin directory."

# Check if rust is installed
$CargoExists = Get-Command cargo -ErrorAction SilentlyContinue
if ($null -eq $CargoExists) {
    Write-Host -ForegroundColor Red "The command line tool `"cargo`" is not available! Please install Rust."
    exit 1
}

# Build project
Write-Host -ForegroundColor DarkMagenta "Running ``cargo build --release``"
cargo build --release
if ($LASTEXITCODE -gt 0) {
    Write-Host -ForegroundColor Red "cargo build exited with non-zero exit code."
    exit $LASTEXITCODE
}


# Copy euphony.exe to ./bin/
Write-Host -ForegroundColor DarkMagenta "Copying binary to ./bin"

$TargetReleaseBinary = Join-Path $PSScriptRoot "target/release/euphony.exe"

$BinDirectory = Join-Path -Path $PSScriptRoot -ChildPath "bin"
Add-Directory -DirectoryPath $BinDirectory

Copy-Item -Path $TargetReleaseBinary -Destination $BinDirectory

# Copy configuration.TEMPLATE.toml to ./bin/data
Write-Host -ForegroundColor DarkMagenta "Copying configuration template to ./bin/data"
$SourceConfigurationTemplate = Join-Path $PSScriptRoot "data/configuration.TEMPLATE.toml"

$BinDataDirectory = Join-Path -Path $BinDirectory -ChildPath "data"
Add-Directory -DirectoryPath $BinDataDirectory

Copy-Item -Path $SourceConfigurationTemplate -Destination $BinDataDirectory


Write-Host ""
Write-Host -ForegroundColor Green "-- BUILD AND COPY COMPLETE --"
Write-Host -ForegroundColor DarkBlue "Make sure you copy ./bin/data/configuration.TEMLPLATE.toml to ./bin/data/configuration.toml and fill out the details."
Write-Host -ForegroundColor DarkMagenta "If you want euphony in your path, add $BinDirectory to your PATH variable."
