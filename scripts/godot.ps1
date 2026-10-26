param(
    [ValidateSet('build','run','demo','editor','smoke','export-debug','export-release','multiplayer')]
    [string]$Action = 'run',
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$GameArgs
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $repoRoot
$godotExe = $env:GODOT_BIN
if (-not $godotExe) { $godotExe = [Environment]::GetEnvironmentVariable('GODOT_BIN','User') }
if (-not $godotExe) { throw 'Set GODOT_BIN to the Godot 4.7.2 executable.' }
$consoleExe = [IO.Path]::Combine([IO.Path]::GetDirectoryName($godotExe), [IO.Path]::GetFileNameWithoutExtension($godotExe) + '_console.exe')
if (Test-Path $consoleExe) { $godotExe = $consoleExe }
$sdkRoot = [Environment]::GetEnvironmentVariable('DOTNET_ROOT','User')
if ($sdkRoot) { $env:DOTNET_ROOT=$sdkRoot; $env:PATH=$sdkRoot+';'+$env:PATH }

function Get-DllDigest([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $hash = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($hash.ComputeHash($stream)) }
    finally { $hash.Dispose(); $stream.Dispose() }
}

function Build-Extension([bool]$Release) {
    $profile = if ($Release) { 'release' } else { 'debug' }
    $buildArgs = @('build','-p','game-godot','--jobs','2')
    if ($Release) { $buildArgs += '--release' }
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) { throw 'Rust extension build failed.' }
    $meta = (& cargo metadata --no-deps --format-version 1 | ConvertFrom-Json)
    $dest = Join-Path $repoRoot "godot/bin/$profile"
    New-Item -ItemType Directory -Force $dest | Out-Null
    $source = Join-Path $meta.target_directory "$profile/game_godot.dll"
    $installed = Join-Path $dest 'game_godot.dll'
    if ((Test-Path $installed) -and (Get-DllDigest $source) -eq (Get-DllDigest $installed)) { return }
    try { Copy-Item -LiteralPath $source -Destination $dest -Force }
    catch { throw "Close the Godot editor and game before replacing the extension DLL. $_" }
}

Build-Extension $false
if ($Action -eq 'build') { exit 0 }
if ($Action -eq 'smoke') {
    & $godotExe --headless --path godot --editor --import --quit
    if ($LASTEXITCODE -ne 0) { throw 'Godot import failed.' }
    & $godotExe --headless --path godot --script res://tests/smoke.gd
    exit $LASTEXITCODE
}
if ($Action -eq 'multiplayer') {
    & cargo build -p server --jobs 2
    if ($LASTEXITCODE -ne 0) { throw 'Server build failed.' }
    $meta = (& cargo metadata --no-deps --format-version 1 | ConvertFrom-Json)
    $testDir = Join-Path $meta.target_directory ('godot-network-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Force $testDir | Out-Null
    # Allocate an available UDP port for the isolated QUIC test.
    $socket = New-Object System.Net.Sockets.UdpClient(0)
    $port = $socket.Client.LocalEndPoint.Port
    $socket.Dispose()
    $env:CDDA_BIND = "127.0.0.1:$port"
    $env:CDDA_TEST_ADDRESS = $env:CDDA_BIND
    $env:CDDA_SAVE_PATH = Join-Path $testDir 'world3d.bin'
    $env:CDDA_CERT_PATH = Join-Path $testDir 'server.cert.der'
    $env:CDDA_RUN_SECONDS = '25'
    $server = Start-Process -FilePath (Join-Path $meta.target_directory 'debug/server.exe') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $testDir 'server.out.log') -RedirectStandardError (Join-Path $testDir 'server.err.log')
    # Retain the native handle so Windows PowerShell can read ExitCode after exit.
    $serverHandle = $server.Handle
    for ($i=0; $i -lt 100 -and -not (Test-Path $env:CDDA_CERT_PATH); $i++) { Start-Sleep -Milliseconds 50 }
    & $godotExe --headless --path godot --script res://tests/multiplayer.gd
    $testExit = $LASTEXITCODE
    # Let the isolated authority save and shut down normally.
    $server.WaitForExit(30000) | Out-Null
    if (-not $server.HasExited) { throw "Test server did not exit: PID $($server.Id)" }
    if ($server.ExitCode -ne 0 -or -not (Test-Path $env:CDDA_SAVE_PATH)) { throw 'Test server failed to save.' }
    Write-Output "Godot network evidence: $testDir"
    exit $testExit
}
if ($Action.StartsWith('export-')) {
    # This project has no C# gameplay; use standard templates even with a .NET editor.
    $templates = Join-Path $env:APPDATA 'Godot/export_templates/4.7.2.stable'
    $stagedTemplates = Join-Path $repoRoot 'target/godot-templates'
    New-Item -ItemType Directory -Force $stagedTemplates | Out-Null
    foreach ($file in @('windows_debug_x86_64.exe','windows_release_x86_64.exe')) {
        $source = Join-Path $templates $file
        if (-not (Test-Path $source)) { throw "Install Godot 4.7.2 standard export templates: $source" }
        Copy-Item -LiteralPath $source -Destination $stagedTemplates -Force
    }
    $release = $Action -eq 'export-release'
    if ($release) { Build-Extension $true }
    $meta = (& cargo metadata --no-deps --format-version 1 | ConvertFrom-Json)
    $folder = Join-Path $meta.target_directory $Action
    New-Item -ItemType Directory -Force $folder | Out-Null
    # Export a clean runtime-only staging project. Editor tools and their settings
    # must not enter project.binary or compete with the live MCP editor's DLLs.
    $exportProject = Join-Path $meta.target_directory ('godot-export-stage-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $exportProject | Out-Null
    foreach ($item in @('project.godot','game.gdextension','game.gdextension.uid','export_presets.cfg','bin','materials','scenes','src')) {
        $source = Join-Path $repoRoot "godot/$item"
        if (Test-Path $source) { Copy-Item -LiteralPath $source -Destination $exportProject -Recurse }
    }
    $utf8 = New-Object System.Text.UTF8Encoding($false)
    $projectFile = Join-Path $exportProject 'project.godot'
    $projectText = [IO.File]::ReadAllText($projectFile)
    $projectText = [regex]::Replace($projectText, '(?ms)^\[(?:dotnet|editor_plugins)\]\r?\n.*?(?=^\[|\z)', '')
    [IO.File]::WriteAllText($projectFile, $projectText, $utf8)
    $presetFile = Join-Path $exportProject 'export_presets.cfg'
    $presetText = [IO.File]::ReadAllText($presetFile).Replace('../target/godot-templates/', $stagedTemplates.Replace('\','/') + '/')
    [IO.File]::WriteAllText($presetFile, $presetText, $utf8)
    # Load the native extension at startup instead of discovering/loading it
    # midway through the first import (which can crash Godot on shutdown).
    $stageCache = Join-Path $exportProject '.godot'
    New-Item -ItemType Directory -Path $stageCache | Out-Null
    [IO.File]::WriteAllText((Join-Path $stageCache 'extension_list.cfg'), "res://game.gdextension`n", $utf8)
    & $godotExe --headless --path $exportProject --editor --import --quit
    if ($LASTEXITCODE -ne 0) { throw 'Godot import failed.' }
    $flag = '--' + $Action
    & $godotExe --headless --path $exportProject $flag 'Windows Desktop' (Join-Path $folder 'Dungeon.exe')
    exit $LASTEXITCODE
}
if ($Action -eq 'editor') { & $godotExe --path godot --editor; exit $LASTEXITCODE }
$launchArgs = @('--path','godot','--')
if ($Action -eq 'demo') { $launchArgs += '--demo' }
$launchArgs += $GameArgs
& $godotExe @launchArgs
exit $LASTEXITCODE
