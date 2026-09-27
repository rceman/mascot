# Build the Go mascot candidate, including the application manifest resource
# step (SDK rc.exe -> gen_syso.go -> rsrc_windows_amd64.syso linked by Go).
# MSVC cvtres.exe cannot be used: it emits .debug$S/@comp.id absolute symbols
# that the Go linker rejects with "sectnum < 0".
# Usage: powershell -ExecutionPolicy Bypass -File build.ps1
$ErrorActionPreference = 'Stop'
$env:GOTOOLCHAIN = 'local'
$env:CGO_ENABLED = '0'
$rc = 'C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe'
if (-not (Test-Path $rc)) { throw "pinned rc.exe not found: $rc" }
& $rc /nologo /fo app.res app.rc
if ($LASTEXITCODE -ne 0) { throw "rc.exe failed: $LASTEXITCODE" }
go run gen_syso.go app.res rsrc_windows_amd64.syso
if ($LASTEXITCODE -ne 0) { throw "gen_syso failed: $LASTEXITCODE" }
New-Item -ItemType Directory -Force ..\out\go | Out-Null
go test ./...
if ($LASTEXITCODE -ne 0) { throw "go test failed: $LASTEXITCODE" }
go build -trimpath -buildvcs=false -ldflags='-H windowsgui -s -w -buildid=' -o '..\out\go\mascot.exe' .
if ($LASTEXITCODE -ne 0) { throw "go build failed: $LASTEXITCODE" }
