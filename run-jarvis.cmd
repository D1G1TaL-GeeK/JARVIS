@echo off
setlocal

pushd "%~dp0"

set "JARVIS_CARGO="

where cargo >nul 2>nul
if %ERRORLEVEL%==0 (
    set "JARVIS_CARGO=cargo"
)

if not defined JARVIS_CARGO if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "JARVIS_CARGO=%USERPROFILE%\.cargo\bin\cargo.exe"
)

if not defined JARVIS_CARGO (
    for /d %%D in ("%ProgramFiles%\Rust *") do (
        if not defined JARVIS_CARGO if exist "%%~fD\bin\cargo.exe" set "JARVIS_CARGO=%%~fD\bin\cargo.exe"
    )
)

if not defined JARVIS_CARGO if defined ProgramFiles(x86) (
    for /d %%D in ("%ProgramFiles(x86)%\Rust *") do (
        if not defined JARVIS_CARGO if exist "%%~fD\bin\cargo.exe" set "JARVIS_CARGO=%%~fD\bin\cargo.exe"
    )
)

if not defined JARVIS_CARGO (
    echo ERROR: cargo.exe WAS NOT FOUND.
    echo ERROR: INSTALL RUST OR OPEN A TERMINAL WHERE cargo IS AVAILABLE.
    popd
    endlocal & exit /b 9009
)

"%JARVIS_CARGO%" run
set "JARVIS_EXIT_CODE=%ERRORLEVEL%"

popd
endlocal & exit /b %JARVIS_EXIT_CODE%
