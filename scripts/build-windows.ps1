#Requires -Version 5.1
<#
.SYNOPSIS
    Build de production de Twitch Voice Reader pour Windows (x64).

.DESCRIPTION
    1. Vérifie les prérequis (Node.js, Rust, cible MSVC).
    2. Installe les dépendances npm.
    3. Télécharge le binaire Piper + voix par défaut dans
       `src-tauri/resources/piper/` (bundlé dans l'installeur final).
    4. Lance `tauri build` (génère un installeur NSIS + MSI).

.NOTES
    Nécessite TWITCH_CLIENT_ID défini en variable d'environnement
    (voir README.md, section "Configuration développeur").
#>

$ErrorActionPreference = "Stop"

Write-Host "== Twitch Voice Reader — Build Windows ==" -ForegroundColor Cyan

if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    throw "Node.js est requis (https://nodejs.org)."
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "Rust/Cargo est requis (https://rustup.rs)."
}
if (-not $env:TWITCH_CLIENT_ID) {
    Write-Warning "TWITCH_CLIENT_ID n'est pas défini : l'authentification Twitch échouera à l'exécution."
}

Write-Host "-> Installation des dépendances npm..." -ForegroundColor Yellow
npm ci

Write-Host "-> Récupération du moteur Piper (Windows x64)..." -ForegroundColor Yellow
& "$PSScriptRoot\install-piper.ps1" -Platform "windows-x64"

Write-Host "-> Build Tauri (release)..." -ForegroundColor Yellow
npm run tauri build -- --target x86_64-pc-windows-msvc

Write-Host "== Build terminé =="  -ForegroundColor Green
Write-Host "Installeurs générés dans src-tauri/target/x86_64-pc-windows-msvc/release/bundle/"
