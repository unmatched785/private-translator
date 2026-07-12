@echo off
setlocal

set "APP=%~dp0PrivateTranslator-Lite.exe"
if not exist "%APP%" set "APP=%~dp0PrivateTranslator-Quality.exe"

if not exist "%APP%" (
  echo Private Translator executable was not found next to this installer.
  echo Extract the complete ZIP before running Install-Model.cmd.
  pause
  exit /b 1
)

echo Private Translator will install the pinned official model for this Windows account.
echo The one-time download is about 1.13 GB. Interrupted downloads can be resumed.
echo.
"%APP%" setup %*
set "CODE=%ERRORLEVEL%"

echo.
if "%CODE%"=="0" (
  echo Model setup finished successfully.
) else (
  echo Model setup failed with exit code %CODE%.
)
pause
exit /b %CODE%
