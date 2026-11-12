$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'bin\GridePOS-Cotizaciones.exe'
if (-not (Test-Path -LiteralPath $exe)) { throw 'No se encontro el ejecutable junto al proyecto.' }
$desktop = [Environment]::GetFolderPath('Desktop')
$path = Join-Path $desktop 'GridePOS - Cotizaciones y facturas.lnk'
if (Test-Path -LiteralPath $path) { throw 'Ya existe ese acceso directo. Se conserva sin cambios.' }
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($path)
$link.TargetPath = $exe
$link.WorkingDirectory = $PSScriptRoot
$link.Description = 'Cotizaciones y facturas GridePOS sin conexion'
$link.Save()
Write-Host 'Acceso directo creado.'
