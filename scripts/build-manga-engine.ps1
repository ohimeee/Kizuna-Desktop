# Builds the manga engine download: a jlink-trimmed Java runtime + the extension host jar,
# zipped as dist\kizuna-manga-engine-<version>.zip, with its SHA-256 printed for
# src-tauri\src\manga\engine.rs. The zip is attached to the app's GitHub release.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-manga-engine.ps1 -Version 1
#
# Needs a full JDK 25 with jlink for the runtime (Eclipse Temurin; Android Studio's JBR can't jlink).
param(
    [Parameter(Mandatory = $true)][int]$Version,
    [string]$Jdk = "$env:USERPROFILE\tools\temurin-25",
    # Gradle keeps using the JDK it was set up with.
    [string]$GradleJdk = "C:\Program Files\Android\Android Studio\jbr"
)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$dist = Join-Path $root "dist"
$stage = Join-Path $dist "manga-engine"
$zip = Join-Path $dist "kizuna-manga-engine-$Version.zip"

# Modules the host needs (jdeps on the fat jar), minus Graal's jdk.internal.vm.ci and jdk.jfr,
# plus what extensions load at runtime: extra charsets (CJK sites), locale data (date parsing
# in many languages) and zipfs.
$modules = "java.base,java.desktop,java.instrument,java.naming,java.net.http,java.prefs,java.scripting," +
    "java.sql,jdk.management,jdk.unsupported,jdk.charsets,jdk.localedata,jdk.zipfs"

Write-Host "Building the extension host jar"
$env:JAVA_HOME = $GradleJdk
Push-Location (Join-Path $root "ext-host")
try {
    # --rerun: Gradle doesn't see fatJar's exclude list as an input.
    & .\gradlew.bat :host:fatJar --rerun --console=plain -q
    if ($LASTEXITCODE -ne 0) { throw "gradle failed" }
} finally { Pop-Location }

Write-Host "Linking the runtime"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
& "$Jdk\bin\jlink.exe" --add-modules $modules --strip-debug --no-header-files --no-man-pages `
    --compress zip-6 --generate-cds-archive --output (Join-Path $stage "runtime")
if ($LASTEXITCODE -ne 0) { throw "jlink failed" }
# Class cache for heaps over 32 GB (the host is capped at 768 MB) and the native link library.
Remove-Item (Join-Path $stage "runtime\bin\server\classes_nocoops.jsa"), (Join-Path $stage "runtime\lib\jvm.lib") -ErrorAction SilentlyContinue

Copy-Item (Join-Path $root "ext-host\host\build\libs\kizuna-ext-host.jar") $stage
Copy-Item (Join-Path $root "ext-host\NOTICE.md") $stage
Set-Content -Path (Join-Path $stage "version.txt") -Value $Version -Encoding ascii -NoNewline

Write-Host "Zipping"
if (Test-Path $zip) { Remove-Item $zip }
Push-Location $stage
try {
    & "$env:SystemRoot\System32\tar.exe" -a -cf $zip runtime kizuna-ext-host.jar NOTICE.md version.txt
    if ($LASTEXITCODE -ne 0) { throw "tar failed" }
} finally { Pop-Location }

$item = Get-Item $zip
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
Write-Host ""
Write-Host ("{0}  {1:N1} MB" -f $item.Name, ($item.Length / 1MB))
Write-Host "ENGINE_VERSION = $Version"
Write-Host "ENGINE_SIZE    = $($item.Length)"
Write-Host "ENGINE_SHA256  = $hash"
