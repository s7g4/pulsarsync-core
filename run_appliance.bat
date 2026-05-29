@echo off
title PulsarSync SDR-Appliance Orchestrator
echo =====================================================================
echo           PULSARSYNC SDR-APPLIANCE AUTOMATED LAUNCHER
echo =====================================================================
echo.

echo [1/4] Booting Rust Host Daemon (Receiver & DSP) in separate window...
:: Launches cargo run in a separate CMD window so log outputs are isolated
start "PulsarSync Ingestion Daemon" cmd /k "cargo run --features host-testing --target x86_64-pc-windows-msvc"

echo [2/4] Waiting 4 seconds for HTTP server to initialize...
timeout /t 4 /nobreak > nul

echo [3/4] Launching Web Telemetry Dashboard in your default browser...
start http://localhost:8082

echo [4/4] Starting Python VITA-49 Stream Emitter in this window...
echo.
echo =====================================================================
echo   Running Stream Emitter... Press Ctrl+C to stop the emitter here.
echo   To stop the Rust Daemon, close the separate CMD console window.
echo =====================================================================
echo.
python -u scripts/stream_emitter.py

pause
