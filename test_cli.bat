@echo off
echo ===================================================
echo   PurgeKit - Automatic CLI Verification Script
echo ===================================================
echo.

echo [1/4] Compiling Rust binary in debug mode...
cargo build --manifest-path src-tauri/Cargo.toml
if %errorlevel% neq 0 (
    echo Error: Compilation failed!
    exit /b %errorlevel%
)
echo.

echo [2/4] Setting up mock application for testing...
echo Creating registry key: HKCU\Software\PurgeKitTestApp
reg add HKCU\Software\PurgeKitTestApp /f > nul
if %errorlevel% neq 0 (
    echo Error: Failed to create test registry key!
    exit /b %errorlevel%
)

echo Creating AppData folder: %%AppData%%\PurgeKitTestApp
mkdir "%AppData%\PurgeKitTestApp" 2> nul
echo Mock data > "%AppData%\PurgeKitTestApp\dummy.txt"
echo.

echo [3/4] Running PurgeKit CLI to clean mock application...
src-tauri\target\debug\purgekit-cli.exe clean PurgeKitTestApp -y --min-score 50
echo.

echo [4/4] Verifying if remnants were successfully purged...
reg query HKCU\Software\PurgeKitTestApp > nul 2>&1
if %errorlevel% equ 0 (
    echo [FAIL] Registry key HKCU\Software\PurgeKitTestApp still exists!
    set TEST_FAILED=1
) else (
    echo [PASS] Registry key HKCU\Software\PurgeKitTestApp was deleted.
)

if exist "%AppData%\PurgeKitTestApp" (
    echo [FAIL] Directory %%AppData%%\PurgeKitTestApp still exists!
    set TEST_FAILED=1
) else (
    echo [PASS] Directory %%AppData%%\PurgeKitTestApp was deleted.
)
echo.

echo ===================================================
if "%TEST_FAILED%"=="1" (
    echo   VERIFICATION RESULT: FAILED
) else (
    echo   VERIFICATION RESULT: ALL TESTS PASSED
)
echo ===================================================
rem pause
