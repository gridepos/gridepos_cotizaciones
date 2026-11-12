$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot
cargo test --locked
if ($LASTEXITCODE -ne 0) { throw 'Las pruebas fallaron; no se reemplaza el ejecutable.' }
cargo build --release --locked
if ($LASTEXITCODE -ne 0) { throw 'La compilacion fallo.' }
New-Item -ItemType Directory -Path (Join-Path $PSScriptRoot 'bin') -Force | Out-Null
Copy-Item (Join-Path $PSScriptRoot 'target\release\gridepos-cotizaciones.exe') (Join-Path $PSScriptRoot 'bin\GridePOS-Cotizaciones.exe')
Write-Host 'Compilacion lista en bin\GridePOS-Cotizaciones.exe'
