@echo off
setlocal
cd /d "%~dp0"
"%~dp0vimit.exe" --monitor %*
echo.
pause
