@echo off
rem Runs the Hivemind release binary, building it first if it does not exist yet.
setlocal
cd /d "%~dp0"

set EXE=src-tauri\target\release\hivemind.exe
if not exist "%EXE%" (
  call build.bat || exit /b 1
)
start "" "%EXE%" %*
