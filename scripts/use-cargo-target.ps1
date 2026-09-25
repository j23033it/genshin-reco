$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path.TrimEnd('\', '/')
$pathBytes = [System.Text.Encoding]::UTF8.GetBytes($projectRoot.ToUpperInvariant())
$pathHash = [Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($pathBytes)).Substring(0, 12).ToLowerInvariant()
$env:CARGO_TARGET_DIR = Join-Path $env:USERPROFILE ".cache/genshin-reco/target-$pathHash"
Write-Output "Cargo ビルド先: $env:CARGO_TARGET_DIR"
