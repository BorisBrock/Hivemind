@echo off
rem Runs the Workzone release binary, building it first if it does not exist yet.
setlocal
cd /d "%~dp0"

set EXE=src-tauri\target\release\workzone.exe
if not exist "%EXE%" (
  call build.bat || exit /b 1
)
start "" "%EXE%" %*
