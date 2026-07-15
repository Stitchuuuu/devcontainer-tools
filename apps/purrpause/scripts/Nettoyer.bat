@echo off
setlocal
title Nettoyer completement PurrPause

:: Auto-elevate to admin — service delete + task delete + programdata rm need it.
NET SESSION >nul 2>&1
if %errorlevel% NEQ 0 (
  echo Elevation requise, ouverture UAC...
  powershell -NoProfile -Command "Start-Process '%~f0' -Verb RunAs"
  exit /b
)

echo.
echo === PurrPause : nettoyage complet ===
echo.
echo Ce script va :
echo   1. Arreter puis desinstaller le service WindowsSystemHealth
echo   2. Supprimer la tache planifiee \Microsoft\Windows\SystemHealth\HealthCheck
echo   3. Effacer le dossier C:\ProgramData\DiagnosticsCache (config + logs + passcode)
echo.
echo L'exe SystemHealthAgent.exe et son manifest restent sur disque
echo (a toi de les supprimer manuellement si tu veux desinstaller definitivement).
echo.
choice /C ON /N /M "Continuer ? [O]ui / [N]on : "
if errorlevel 2 (
  echo Annule.
  pause
  exit /b 1
)

echo.
echo [1/3] Arret + suppression du service...
sc.exe stop WindowsSystemHealth 2>nul
sc.exe delete WindowsSystemHealth 2>nul

echo.
echo [2/3] Suppression de la tache planifiee...
schtasks /delete /tn "\Microsoft\Windows\SystemHealth\HealthCheck" /f 2>nul

echo.
echo [3/3] Suppression du dossier de configuration...
if exist "C:\ProgramData\DiagnosticsCache" (
  rmdir /S /Q "C:\ProgramData\DiagnosticsCache"
)

echo.
echo === Termine ===
echo Verifs :
sc.exe query WindowsSystemHealth 2>nul
if %errorlevel% NEQ 0 echo   [OK] Service absent.
schtasks /query /tn "\Microsoft\Windows\SystemHealth\HealthCheck" 2>nul
if %errorlevel% NEQ 0 echo   [OK] Tache absente.
if not exist "C:\ProgramData\DiagnosticsCache" echo   [OK] Dossier config absent.

echo.
pause
