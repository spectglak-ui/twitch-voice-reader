#Requires -Version 5.1
<#
.SYNOPSIS
    Télécharge le binaire Piper (Windows x64) et la voix française par
    défaut dans src-tauri/resources/piper/.

.NOTES
    Ce script reste utile pour PACKAGER Piper à l'intérieur de
    l'installeur final (afin qu'un utilisateur final n'ait besoin d'aucun
    accès réseau au premier lancement). L'application elle-même sait
    désormais aussi télécharger Piper automatiquement au premier
    lancement si absent (voir src-tauri/src/tts/installer.rs) — ce script
    n'est donc plus un prérequis strict pour le développement local,
    seulement pour un packaging "prêt à l'emploi hors-ligne".

    Le dépôt source de Piper (rhasspy/piper) a été archivé par son
    propriétaire (lecture seule depuis octobre 2025) ; le développement se
    poursuit sous OHF-Voice/piper1-gpl, qui ne publie plus d'exécutable
    Windows autonome. Ce script cible donc délibérément la dernière
    version figée (2023.11.14-2) — les anciens artefacts restent
    téléchargeables malgré l'archivage.
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Platform
)

$ErrorActionPreference = "Stop"

# Force TLS 1.2 : sur Windows PowerShell 5.1 (par opposition à PowerShell 7+),
# .NET n'active pas toujours TLS 1.2 par défaut selon la configuration système,
# ce qui fait échouer silencieusement (ou avec une erreur peu claire de type
# "Could not create SSL/TLS secure channel") toute requête vers GitHub, qui
# exige TLS 1.2 au minimum. C'est une cause fréquente et facile à manquer
# d'échec de téléchargement sur des installations Windows par défaut.
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$RootDir = Split-Path -Parent $PSScriptRoot
$DestDir = Join-Path $RootDir "src-tauri\resources\piper"
$PiperVersion = "2023.11.14-2"
# En dessous de ce seuil, la réponse est presque certainement une page
# d'erreur HTML plutôt que l'archive réelle (plusieurs dizaines de Mo).
$MinPlausibleBytes = 5000000

New-Item -ItemType Directory -Force -Path $DestDir | Out-Null

if (-not (Test-Path "$DestDir\piper.exe")) {
    Write-Host "-> Téléchargement de Piper $PiperVersion (Windows x64)..."
    $asset = "piper_windows_amd64.zip"
    $url = "https://github.com/rhasspy/piper/releases/download/$PiperVersion/$asset"
    $zipPath = Join-Path $env:TEMP "$([guid]::NewGuid()).zip"

    try {
        Invoke-WebRequest -Uri $url -OutFile $zipPath -UseBasicParsing
    } catch {
        Write-Error "Échec du téléchargement depuis $url : $($_.Exception.Message)"
        Write-Error "Vérifiez votre connexion réseau et, si vous êtes derrière un proxy d'entreprise, sa configuration."
        exit 1
    }

    $actualSize = (Get-Item $zipPath).Length
    if ($actualSize -lt $MinPlausibleBytes) {
        Remove-Item $zipPath -ErrorAction SilentlyContinue
        Write-Error "Fichier téléchargé anormalement petit ($actualSize octets) : probablement une page d'erreur plutôt que l'archive Piper."
        exit 1
    }

    try {
        Expand-Archive -Path $zipPath -DestinationPath $DestDir -Force
    } catch {
        Write-Error "Échec de l'extraction de l'archive : $($_.Exception.Message)"
        exit 1
    } finally {
        Remove-Item $zipPath -ErrorAction SilentlyContinue
    }

    if (-not (Test-Path "$DestDir\piper.exe")) {
        # Filet de sécurité : recherche récursive au cas où la structure de
        # l'archive changerait un jour (dossier imbriqué), plutôt que de
        # laisser un dossier vide sans explication — c'est exactement le
        # symptôme initialement rencontré.
        $found = Get-ChildItem -Path $DestDir -Filter "piper.exe" -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found) {
            Move-Item $found.FullName "$DestDir\piper.exe" -Force
        } else {
            Write-Error "L'archive a été extraite mais aucun 'piper.exe' n'a été trouvé. Contenu extrait :"
            Get-ChildItem -Path $DestDir -Recurse | ForEach-Object { Write-Error "  $($_.FullName)" }
            exit 1
        }
    }

    # Smoke test : confirme que le binaire s'exécute réellement plutôt que
    # de découvrir un problème (mauvaise architecture, binaire corrompu,
    # blocage antivirus) bien plus tard, silencieusement.
    try {
        & "$DestDir\piper.exe" --version | Out-Null
    } catch {
        Write-Warning "Le binaire Piper téléchargé ne semble pas s'exécuter correctement sur cette machine : $($_.Exception.Message)"
    }
} else {
    Write-Host "-> Binaire Piper déjà présent, téléchargement ignoré."
}

$VoicesDir = Join-Path $DestDir "voices"
New-Item -ItemType Directory -Force -Path $VoicesDir | Out-Null
$DefaultVoice = "fr_FR-siwis-medium"
$BaseUrl = "https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/siwis/medium"

if (-not (Test-Path "$VoicesDir\$DefaultVoice.onnx")) {
    Write-Host "-> Téléchargement de la voix par défaut ($DefaultVoice)..."
    try {
        Invoke-WebRequest -Uri "$BaseUrl/$DefaultVoice.onnx" -OutFile "$VoicesDir\$DefaultVoice.onnx" -UseBasicParsing
        Invoke-WebRequest -Uri "$BaseUrl/$DefaultVoice.onnx.json" -OutFile "$VoicesDir\$DefaultVoice.onnx.json" -UseBasicParsing
    } catch {
        Write-Error "Échec du téléchargement de la voix par défaut : $($_.Exception.Message)"
        exit 1
    }

    $voiceSize = (Get-Item "$VoicesDir\$DefaultVoice.onnx").Length
    if ($voiceSize -lt 1000000) {
        Write-Error "Modèle de voix téléchargé anormalement petit ($voiceSize octets)."
        exit 1
    }
}

Write-Host "-> Piper installé et vérifié dans $DestDir" -ForegroundColor Green
