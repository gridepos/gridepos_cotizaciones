@echo off
if exist "%~dp0bin\GridePOS-Cotizaciones.exe" (
  start "" "%~dp0bin\GridePOS-Cotizaciones.exe"
) else if exist "%~dp0GridePOS-Cotizaciones.exe" (
  start "" "%~dp0GridePOS-Cotizaciones.exe"
) else (
  echo No se encontro el programa. Ejecuta COMPILAR-WINDOWS.ps1 primero.
  pause
  exit /b 1
)
