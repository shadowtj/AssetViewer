@echo off
setlocal

REM ============================================
REM Asset Viewer - project bootstrap (Windows)
REM ============================================

REM 1) Ensure Rust is available
where cargo >nul 2>nul
if errorlevel 1 (
  echo [ERROR] Cargo (Rust) not found in PATH.
  echo Install Rust from https://rustup.rs/ and reopen this terminal.
  exit /b 1
)

REM 2) Build once so dependencies are downloaded
pushd "%~dp0..\asset_viewer"
echo [INFO] Running cargo build...
cargo build
if errorlevel 1 (
  echo [ERROR] Build failed.
  popd
  exit /b 1
)

echo [OK] Project ready.
echo You can run: scripts\run_dev.bat
popd
endlocal
