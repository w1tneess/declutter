@echo off
rem Builds dist\Declutter (onedir), dist\Declutter-portable.zip and, if Inno Setup is installed, dist\Declutter-Setup.exe.
rem Usage: set APP_VERSION=1.0.0 && build.bat
setlocal
cd /d "%~dp0"
if "%APP_VERSION%"=="" set APP_VERSION=0.0.0-dev

python -m pip install -r requirements-dev.txt || goto :fail
python -m pytest -q || goto :fail

python -m PyInstaller --noconfirm --clean --onedir --windowed --name Declutter ^
  --icon "%~dp0assets\icon.ico" --paths "%~dp0src" --collect-data customtkinter ^
  --specpath build --workpath build\pyi --distpath dist "%~dp0launcher.py" || goto :fail

powershell -NoProfile -Command "Compress-Archive -Path dist\Declutter -DestinationPath dist\Declutter-portable.zip -Force" || goto :fail

set ISCC=
where iscc >nul 2>nul && set ISCC=iscc
if "%ISCC%"=="" if exist "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe" set ISCC="%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
if "%ISCC%"=="" (
  echo Inno Setup not found, skipping the installer. Install it from https://jrsoftware.org/isinfo.php
) else (
  %ISCC% /DAppVersion=%APP_VERSION% installer\declutter.iss || goto :fail
)
echo Done. Output is in dist\
exit /b 0

:fail
echo BUILD FAILED
exit /b 1
