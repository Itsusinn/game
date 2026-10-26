param([string]$ServerUrl = 'http://localhost:17860')
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path $PSScriptRoot -Parent
$projectPath = Join-Path $repoRoot 'godot'
$godotExe = $env:GODOT_BIN
if (-not $godotExe) { $godotExe = [Environment]::GetEnvironmentVariable('GODOT_BIN','User') }
if (-not $godotExe) { throw 'Set GODOT_BIN to the Godot 4.7.2 .NET executable.' }
$sdkRoot = $env:DOTNET_ROOT
if (-not $sdkRoot) { $sdkRoot = [Environment]::GetEnvironmentVariable('DOTNET_ROOT','User') }
if ($sdkRoot) { $env:DOTNET_ROOT = $sdkRoot; $env:PATH = $sdkRoot + ';' + $env:PATH }
if (-not (Get-Command godot-cli -ErrorAction SilentlyContinue)) { throw 'Install godot-cli: npm install -g godot-cli@0.33.0' }
if (-not (Test-Path (Join-Path $projectPath 'addons/godot_mcp/plugin.cfg'))) {
    & godot-cli install-plugin $projectPath --version 0.33.0
    if ($LASTEXITCODE -ne 0) { throw 'Godot MCP addon installation failed.' }
}
$localPackages = Join-Path (Split-Path $godotExe -Parent) 'GodotSharp/Tools/nupkgs'
$editorProject = Join-Path $projectPath 'Dungeon.Editor.csproj'
& dotnet restore $editorProject --source $localPackages --source https://api.nuget.org/v3/index.json
if ($LASTEXITCODE -ne 0) { throw 'Editor tool dependency restore failed.' }
& dotnet build $editorProject --no-restore --nologo --verbosity quiet
if ($LASTEXITCODE -ne 0) { throw 'Editor tool build failed. Close the project editor before rebuilding.' }
# Build/stage the native DLL separately with just build before opening this editor.
& godot-cli open $projectPath --editor-path $godotExe --headless --no-build --idle-timeout 0 --url $ServerUrl
exit $LASTEXITCODE
