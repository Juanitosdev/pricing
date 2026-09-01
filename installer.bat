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
set "TMP=%TARGET%\TBG.Pricing.tmp"
set "URL=https://github.com/Juanitosdev/pricing/releases/latest/download/TBG.Pricing.exe"

echo.
echo   Instalando TBG Pricing en:
echo   %TARGET%
echo.

if not exist "%TARGET%" mkdir "%TARGET%"

rem Limpia cualquier temporal previo, sea CARPETA o archivo, sin preguntar.
if exist "%TMP%\" rmdir /s /q "%TMP%" 2>nul
if exist "%TMP%" del /f /q "%TMP%" 2>nul

echo   Descargando la ultima version desde GitHub...
rem Usamos curl.exe (incluido en Windows 10/11): mas fiable escribiendo el .exe
rem que Invoke-WebRequest, que daba "Acceso denegado" / dejaba temporales raros.
curl.exe -fL --retry 2 --ssl-no-revoke -o "%TMP%" "%URL%"
if errorlevel 1 goto :dlfail
if not exist "%TMP%" goto :dlfail

rem Coloca el exe: quita cualquier version previa (carpeta o archivo). Si la app
rem esta abierta el exe estara bloqueado y el move fallara -> avisamos.
if exist "%EXE%\" rmdir /s /q "%EXE%" 2>nul
if exist "%EXE%" del /f /q "%EXE%" 2>nul
move /y "%TMP%" "%EXE%" >nul
if errorlevel 1 (
  echo.
  echo   ERROR: no se pudo colocar el exe. Cierra TBG Pricing si esta abierto y reintenta.
  echo.
  if exist "%TMP%" del /f /q "%TMP%" 2>nul
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
(goto) 2>nul & del /f /q "%~f0"

:dlfail
echo.
echo   ERROR: no se pudo descargar la app. Revisa tu conexion e intentalo de nuevo.
echo.
if exist "%TMP%\" rmdir /s /q "%TMP%" 2>nul
if exist "%TMP%" del /f /q "%TMP%" 2>nul
pause
exit /b 1
