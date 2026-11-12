param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\GridePOS-Cotizaciones'),
    [switch]$SinAccesosDirectos
)
$ErrorActionPreference = 'Stop'
$sourceExe = Join-Path $PSScriptRoot 'bin\GridePOS-Cotizaciones.exe'
if (-not (Test-Path -LiteralPath $sourceExe -PathType Leaf)) {
    throw 'No se encontro bin\GridePOS-Cotizaciones.exe. Ejecuta COMPILAR-WINDOWS.ps1 primero.'
}
$installFull = [IO.Path]::GetFullPath($InstallDir)
$dataFull = [IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA 'GridePOS\Cotizaciones'))
if ($installFull.TrimEnd('\').Equals($dataFull.TrimEnd('\'),[StringComparison]::OrdinalIgnoreCase) -or $installFull.StartsWith($dataFull.TrimEnd('\')+'\',[StringComparison]::OrdinalIgnoreCase)) {
    throw 'La carpeta del programa debe ser diferente de la carpeta que guarda los documentos.'
}
New-Item -ItemType Directory -Path $installFull -Force | Out-Null
$destinationExe = Join-Path $installFull 'GridePOS-Cotizaciones.exe'
if (Test-Path -LiteralPath $destinationExe) {
    $running = Get-CimInstance Win32_Process -Filter "Name='GridePOS-Cotizaciones.exe'" | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.Equals($destinationExe,[StringComparison]::OrdinalIgnoreCase) }
    if ($running) { throw 'Cierra la ventana de GridePOS antes de actualizar. Los documentos se conservan.' }
    $archive = Join-Path $installFull ('version-anterior-'+(Get-Date -Format 'yyyyMMdd-HHmmss-fff')+'.exe')
    Copy-Item -LiteralPath $destinationExe -Destination $archive -ErrorAction Stop
}
Copy-Item -LiteralPath $sourceExe -Destination $destinationExe -Force
foreach ($name in @('README.md','GUIA-WINDOWS.md','Abrir-GridePOS.cmd')) {
    $source = Join-Path $PSScriptRoot $name
    if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination (Join-Path $installFull $name) -Force }
}
if (-not $SinAccesosDirectos) {
    $shell = New-Object -ComObject WScript.Shell
    $shortcutName = 'GridePOS - Cotizaciones y facturas.lnk'
    $folders = @([Environment]::GetFolderPath('Desktop'), (Join-Path ([Environment]::GetFolderPath('StartMenu')) 'Programs'))
    foreach ($folder in $folders) {
        $shortcutPath = Join-Path $folder $shortcutName
        if (Test-Path -LiteralPath $shortcutPath) {
            $existing = $shell.CreateShortcut($shortcutPath)
            if (-not $existing.TargetPath.Equals($destinationExe,[StringComparison]::OrdinalIgnoreCase)) {
                Write-Warning ('Se conserva un acceso directo existente: '+$shortcutPath)
                continue
            }
        }
        $link = $shell.CreateShortcut($shortcutPath)
        $link.TargetPath = $destinationExe
        $link.WorkingDirectory = $installFull
        $link.Description = 'Cotizaciones y facturas comerciales locales de GridePOS'
        $link.Save()
    }
}
Write-Host ('Instalado: '+$destinationExe)
Write-Host ('Datos conservados en: '+$dataFull)
Write-Host 'No se requieren permisos de administrador ni internet para usar la aplicacion.'
