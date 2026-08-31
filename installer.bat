@echo off
rem ============================================================================
rem  TBG Pricing - instalador portatil
rem
rem  Coloca este .bat donde quieras (p. ej. el Escritorio) y haz doble clic.
rem  Crea una carpeta "pricing" JUNTO a este archivo, descarga dentro la ultima
rem  version de la app, crea un acceso directo en el Escritorio, la abre y se
rem  autodestruye. Todos los datos que cargue el jefe se guardan en esa carpeta
rem  (pricing\data\), asi que borrar la carpeta = borrarlo todo.
rem ============================================================================
setlocal
title Instalar TBG Pricing

set "TARGET=%~dp0pricing"
set "EXE=%TARGET%\TBG.Pricing.exe"
set "TMP=%TARGET%\TBG.Pricing.exe.download"
set "URL=https://github.com/Juanitosdev/pricing/releases/latest/download/TBG.Pricing.exe"

echo.
echo   Instalando TBG Pricing en:
echo   %TARGET%
echo.

if not exist "%TARGET%" mkdir "%TARGET%"

rem Descarga a un archivo temporal y comprueba el codigo de salida de PowerShell.
rem Las rutas van por variables de entorno ($env:...) para no romper el parseo si
rem el camino contiene apostrofos (p. ej. C:\Users\O'Brien\Desktop).
del "%TMP%" 2>nul
echo   Descargando la ultima version...
set "DL_URL=%URL%"
set "DL_OUT=%TMP%"
powershell -NoProfile -ExecutionPolicy Bypass -Command "try { [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri $env:DL_URL -OutFile $env:DL_OUT -UseBasicParsing } catch { Write-Host $_.Exception.Message; exit 1 }"
if errorlevel 1 goto :dlfail
if not exist "%TMP%" goto :dlfail

rem Descarga correcta: sustituye el exe. Si la app esta abierta el exe estara
rem bloqueado, el move fallara y avisamos (en vez de dejar la version vieja).
del "%EXE%" 2>nul
move /y "%TMP%" "%EXE%" >nul
if errorlevel 1 (
  echo.
  echo   ERROR: no se pudo reemplazar el exe. Cierra TBG Pricing si esta abierto y reintenta.
  echo.
  del "%TMP%" 2>nul
  pause
  exit /b 1
)

echo   Creando acceso directo en el Escritorio...
set "SC_EXE=%EXE%"
set "SC_DIR=%TARGET%"
powershell -NoProfile -Command "$w=New-Object -ComObject WScript.Shell; $s=$w.CreateShortcut([Environment]::GetFolderPath('Desktop')+'\TBG Pricing.lnk'); $s.TargetPath=$env:SC_EXE; $s.WorkingDirectory=$env:SC_DIR; $s.Save()"

echo   Listo. Abriendo TBG Pricing...
start "" "%EXE%"

rem El instalador se autodestruye al terminar.
(goto) 2>nul & del "%~f0"

:dlfail
echo.
echo   ERROR: no se pudo descargar la app. Revisa tu conexion e intentalo de nuevo.
echo.
del "%TMP%" 2>nul
pause
exit /b 1
