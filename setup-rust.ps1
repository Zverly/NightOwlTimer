param(
    [string]$ToolchainRoot = 'E:\NightOwlDev'
)

$ErrorActionPreference = 'Stop'
$env:RUSTUP_HOME = Join-Path $ToolchainRoot 'rustup'
$env:CARGO_HOME = Join-Path $ToolchainRoot 'cargo'
$installer = Join-Path $ToolchainRoot 'rustup-init.exe'
$url = 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe'
New-Item -ItemType Directory -Force -Path $ToolchainRoot, $env:RUSTUP_HOME, $env:CARGO_HOME | Out-Null
Invoke-WebRequest -Uri $url -OutFile $installer
& $installer -y --default-toolchain stable --profile minimal --no-modify-path
& (Join-Path $env:CARGO_HOME 'bin\rustup.exe') component add rustfmt
Remove-Item -LiteralPath $installer -Force

Write-Host "Rust installed under $ToolchainRoot."
Write-Host "Set RUSTUP_HOME=$env:RUSTUP_HOME and CARGO_HOME=$env:CARGO_HOME before building."
