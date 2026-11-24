param(
    [string]$Sdk = $env:ANDROID_HOME,
    [string]$Ndk = $env:ANDROID_NDK_HOME,
    [ValidateSet('arm64-v8a', 'armeabi-v7a', 'x86_64')]
    [string]$Target = 'arm64-v8a',
    [switch]$SkipRust
)

$ErrorActionPreference = 'Stop'
$androidDir = $PSScriptRoot
$repo = Split-Path -Parent $androidDir

if (-not $Sdk) { $Sdk = Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
if (-not $Ndk) {
    $ndkRoot = Join-Path $Sdk 'ndk'
    $Ndk = (Get-ChildItem $ndkRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1).FullName
}
$env:ANDROID_HOME = $Sdk
$env:ANDROID_NDK_HOME = $Ndk

$jbrBin = 'C:\Program Files\Android\Android Studio\jbr\bin'
if (Test-Path $jbrBin) { $env:PATH = "$jbrBin;$env:PATH" }

$buildDir = Join-Path $androidDir 'build'
$jniDir = Join-Path $androidDir 'jniLibs'
New-Item -ItemType Directory -Force -Path $buildDir, $jniDir, (Join-Path $buildDir 'gen') | Out-Null

if (-not $SkipRust) {
    Write-Host "==> cargo ndk build ($Target)"
    Push-Location $repo
    try {
        cargo ndk -t $Target -P 24 -o $jniDir build --release -p paint-android-full
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed: $LASTEXITCODE" }
    } finally { Pop-Location }
}

$libName = 'libpaint_android_full.so'
$libPath = Join-Path $jniDir (Join-Path $Target $libName)
if (-not (Test-Path $libPath)) { throw "Not found: $libPath" }

# Rust cdylib for Android links against libc++_shared.so dynamically; package it too.
$ndkArch = switch ($Target) {
    'arm64-v8a'     { 'aarch64-linux-android' }
    'armeabi-v7a'   { 'arm-linux-androideabi' }
    'x86_64'        { 'x86_64-linux-android' }
}
$libcxx = Join-Path $Ndk "toolchains\llvm\prebuilt\windows-x86_64\sysroot\usr\lib\$ndkArch\libc++_shared.so"
if (-not (Test-Path $libcxx)) { throw "Not found: $libcxx" }
Copy-Item $libcxx (Join-Path $jniDir (Join-Path $Target 'libc++_shared.so')) -Force
$libcxxPath = Join-Path $jniDir (Join-Path $Target 'libc++_shared.so')

$buildTools = Get-ChildItem (Join-Path $Sdk 'build-tools') -Directory | Sort-Object Name -Descending | Select-Object -First 1
$aapt2 = Join-Path $buildTools.FullName 'aapt2.exe'
$zipalign = Join-Path $buildTools.FullName 'zipalign.exe'
$apksigner = Join-Path $buildTools.FullName 'apksigner.bat'
$androidJar = Join-Path $Sdk 'platforms\android-34\android.jar'
if (-not (Test-Path $androidJar)) {
    $androidJar = (Get-ChildItem (Join-Path $Sdk 'platforms') -Directory |
        Where-Object { Test-Path (Join-Path $_.FullName 'android.jar') } |
        Sort-Object Name -Descending | Select-Object -First 1).FullName
    $androidJar = Join-Path $androidJar 'android.jar'
}

$resZip = Join-Path $buildDir 'res.zip'
$unsigned = Join-Path $buildDir 'unsigned.apk'
$aligned = Join-Path $buildDir 'aligned.apk'
$apk = Join-Path $repo 'GrazySAI-android.apk'

Write-Host '==> aapt2 compile/link'
& $aapt2 compile --dir (Join-Path $androidDir 'res') -o $resZip
if ($LASTEXITCODE -ne 0) { throw "aapt2 compile failed: $LASTEXITCODE" }
& $aapt2 link `
    --manifest (Join-Path $androidDir 'AndroidManifest.xml') `
    -I $androidJar `
    -R $resZip `
    --auto-add-overlay `
    --java (Join-Path $buildDir 'gen') `
    --min-sdk-version 24 `
    --target-sdk-version 34 `
    -o $unsigned
if ($LASTEXITCODE -ne 0) { throw "aapt2 link failed: $LASTEXITCODE" }

Write-Host '==> add native library'
$pyScript = Join-Path $buildDir 'add_lib.py'
@"
import sys, zipfile
apk, lib, libcxx, target = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
with zipfile.ZipFile(apk, 'a', compression=zipfile.ZIP_DEFLATED) as z:
    z.write(lib, 'lib/' + target + '/libpaint_android_full.so', compress_type=zipfile.ZIP_STORED)
    z.write(libcxx, 'lib/' + target + '/libc++_shared.so', compress_type=zipfile.ZIP_STORED)
"@ | Set-Content -Encoding utf8 $pyScript
python $pyScript $unsigned $libPath $libcxxPath $Target
if ($LASTEXITCODE -ne 0) { throw "adding lib failed: $LASTEXITCODE" }

Write-Host '==> zipalign'
& $zipalign -f -p 4 $unsigned $aligned
if ($LASTEXITCODE -ne 0) { throw "zipalign failed: $LASTEXITCODE" }

$keystore = Join-Path $env:USERPROFILE '.android\debug.keystore'
if (-not (Test-Path $keystore)) {
    Write-Host '==> create debug keystore'
    & keytool -genkeypair -v -keystore $keystore -alias androiddebugkey `
        -storepass android -keypass android -keyalg RSA -keysize 2048 -validity 10000 `
        -dname 'CN=Android Debug,O=Android,C=US'
}

Write-Host '==> apksigner'
& $apksigner sign --ks $keystore --ks-pass pass:android --key-pass pass:android --out $apk $aligned
if ($LASTEXITCODE -ne 0) { throw "apksigner failed: $LASTEXITCODE" }
& $apksigner verify --verbose $apk

Write-Host "DONE: $apk"
