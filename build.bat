@echo off
rem Builds the Hivemind release binary (no installers) into src-tauri\target\release\.
setlocal
cd /d "%~dp0"

if not exist node_modules (
  call npm install || exit /b 1
)
call npm run tauri build -- --no-bundle || exit /b 1
