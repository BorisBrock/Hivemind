@echo off
rem Builds a Windows release of Hivemind into .release\:
rem   Hivemind-<version>-windows-<arch>.exe   portable executable (runs without installation)
rem   Hivemind-<version>-windows-<arch>.zip   the executable plus README.md
rem Pass --installers to also copy the NSIS and MSI installers there.
setlocal
cd /d "%~dp0"

set "INSTALLERS="
if /i "%~1"=="--installers" set "INSTALLERS=1"

for /f "usebackq delims=" %%v in (`powershell -NoProfile -Command "(Get-Content -Raw src-tauri\tauri.conf.json | ConvertFrom-Json).version"`) do set "VERSION=%%v"
if not defined VERSION (
  echo Could not read the version from src-tauri\tauri.conf.json.
  exit /b 1
)

set "ARCH=x64"
if /i "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "ARCH=arm64"

set "NAME=Hivemind-%VERSION%-windows-%ARCH%"
set "OUT=.release"

if not exist node_modules (
  call npm install || exit /b 1
)

if defined INSTALLERS (
  call npm run tauri build -- --bundles nsis,msi || exit /b 1
) else (
  call npm run tauri build -- --no-bundle || exit /b 1
)

if not exist "%OUT%" mkdir "%OUT%"

copy /y src-tauri\target\release\hivemind.exe "%OUT%\%NAME%.exe" >nul || exit /b 1

set "STAGE=%OUT%\%NAME%"
if exist "%STAGE%" rmdir /s /q "%STAGE%"
mkdir "%STAGE%"
copy /y src-tauri\target\release\hivemind.exe "%STAGE%\Hivemind.exe" >nul || exit /b 1
copy /y README.md "%STAGE%\" >nul || exit /b 1
if exist "%OUT%\%NAME%.zip" del /q "%OUT%\%NAME%.zip"
powershell -NoProfile -Command "Compress-Archive -Path '%STAGE%' -DestinationPath '%OUT%\%NAME%.zip'" || exit /b 1
rmdir /s /q "%STAGE%"

if defined INSTALLERS (
  copy /y src-tauri\target\release\bundle\nsis\*_%VERSION%_*.exe "%OUT%\" >nul || exit /b 1
  copy /y src-tauri\target\release\bundle\msi\*_%VERSION%_*.msi "%OUT%\" >nul || exit /b 1
)

echo.
echo Release files in %CD%\%OUT%:
dir /b "%OUT%"
