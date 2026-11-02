#!/bin/bash
# tools/win-tests.sh USER@HOST [ssh-ключ] — все тесты на настоящей Windows (OpenSSH): сборка здесь, запуск там.
# Тесты ищут данные по пути сборки (env!("CARGO_MANIFEST_DIR")). На Windows путь с «/» считается от корня
# текущего диска, поэтому исходники кладём в %USERPROFILE%\GrazySAI-test\root<этот путь>, а корень
# подключаем диском T: через subst. Ничего вне GrazySAI-test не создаётся.
set -euo pipefail
H=$1; O=(-o ConnectTimeout=10); [ $# -gt 1 ] && O+=(-i "$2" -o IdentitiesOnly=yes)
cd "$(dirname "$0")/.."
T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
cargo test --release --workspace --target x86_64-pc-windows-gnullvm --no-run --message-format=json 2>/dev/null |
  python3 -c 'import sys,json
for l in sys.stdin:
    d=json.loads(l)
    if d.get("reason")=="compiler-artifact" and d.get("executable") and d["profile"]["test"]: print(d["executable"])' >"$T/list"
tar --exclude=./target --exclude=./dist --exclude=./.git --exclude=./fuzz -cf "$T/repo.tar" .
W=$(pwd | tr / '\\')
cat >"$T/run.ps1" <<PS
[Console]::OutputEncoding=[Text.Encoding]::UTF8
\$failed=0
foreach(\$b in 'vulkan','dx12'){ \$env:WGPU_BACKEND=\$b
  foreach(\$e in Get-ChildItem "\$env:USERPROFILE\GrazySAI-test\alltests\*.exe"){
    if(\$b -eq 'dx12' -and \$e.Name -notmatch '^GrazySAI-'){continue}
    \$o=& \$e.FullName 2>&1 | Out-String
    if(\$LASTEXITCODE -ne 0){\$failed++; "\$b \$(\$e.BaseName)"; \$o}
    else{ "\$b \$(\$e.BaseName): " + ((\$o -split "\`n" | Select-String '^test result' | Select-Object -Last 1)) }
  }
}
"FAILED: \$failed"
exit \$failed
PS
cat >"$T/run.bat" <<BAT
@echo off
set R=%USERPROFILE%\GrazySAI-test\root
if not exist "%R%$W" mkdir "%R%$W"
tar -xf "%USERPROFILE%\GrazySAI-test\repo.tar" -C "%R%$W"
subst T: /d >nul 2>&1
subst T: "%R%"
T:
cd "$W"
powershell -NoProfile -ExecutionPolicy Bypass -File "%USERPROFILE%\GrazySAI-test\run.ps1"
set E=%ERRORLEVEL%
C:
subst T: /d
exit /b %E%
BAT
ssh "${O[@]}" "$H" 'if not exist GrazySAI-test\alltests mkdir GrazySAI-test\alltests & del /q GrazySAI-test\alltests\*.exe 2>nul'
scp "${O[@]}" -q $(cat "$T/list") "$H:GrazySAI-test/alltests/"
scp "${O[@]}" -q "$T/repo.tar" "$T/run.ps1" "$T/run.bat" "$H:GrazySAI-test/"
ssh "${O[@]}" "$H" 'GrazySAI-test\run.bat'
