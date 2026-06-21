@echo off
setlocal
cd /d "%~dp0"
"%~dp0nglimit.exe" %*
echo.
pause
