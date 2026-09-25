param(
  [Parameter(Mandatory = $true)]
  [ValidateSet('prepare', 'publish')]
  [string]$Stage,
  [switch]$ConfirmedTested
)

$ErrorActionPreference = 'Stop'
$repo = 'j23033it/genshin-reco-releases'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$tauriConfig = Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
$package = Get-Content -LiteralPath (Join-Path $projectRoot 'package.json') -Raw | ConvertFrom-Json
$cargoManifest = Get-Content -LiteralPath (Join-Path $projectRoot 'src-tauri/Cargo.toml') -Raw
$cargoVersion = [regex]::Match($cargoManifest, '(?m)^version = "([^"]+)"').Groups[1].Value
$version = $tauriConfig.version
$tag = "app-v$version"
$previousSigningKey = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', 'Process')
$previousTargetDir = [Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR', 'Process')

if ($version -ne $package.version -or $version -ne $cargoVersion) {
  throw 'tauri.conf.json、package.json、Cargo.toml の版番号が一致しません。'
}
if ($version -notmatch '^\d+\.\d+\.\d+$') {
  throw '正式版の版番号は x.y.z 形式にしてください。'
}

Push-Location $projectRoot
try {
  if ((git branch --show-current).Trim() -ne 'main') {
    throw '正式版の操作は main ブランチからだけ行えます。'
  }
  if (git status --porcelain) {
    throw '未コミット変更があります。正式版の操作前に確認してください。'
  }
  $head = (git rev-parse HEAD).Trim()
  $remoteHead = (git rev-parse origin/main).Trim()
  if ($head -ne $remoteHead) {
    throw 'ローカル main と GitHub の main が一致しません。'
  }

  if ($Stage -eq 'publish') {
    if (-not $ConfirmedTested) {
      throw '実機確認後に -ConfirmedTested を付けて公開してください。'
    }
    $release = gh release view $tag --repo $repo --json isDraft,assets | ConvertFrom-Json
    if (-not $release.isDraft) {
      throw "${tag} は下書きではありません。公開済み版は上書きしません。"
    }
    $names = @($release.assets | ForEach-Object { $_.name })
    if ('latest.json' -notin $names -or "genshin-reco_${version}_x64-setup.exe" -notin $names) {
      throw '公開に必要な更新情報とインストーラーが揃っていません。'
    }
    gh release edit $tag --repo $repo --draft=false --latest
    if ($LASTEXITCODE -ne 0) { throw 'GitHubで正式版を公開できませんでした。' }
    Write-Output "正式版を公開しました: https://github.com/$repo/releases/tag/$tag"
    return
  }

  gh release view $tag --repo $repo --json id *> $null
  if ($LASTEXITCODE -eq 0) {
    throw "${tag} は既に存在します。古い公開物は上書きしません。"
  }

  $key = Join-Path $env:USERPROFILE '.config/genshin-reco/updater.key'
  if (-not (Test-Path -LiteralPath $key)) {
    throw "署名用の秘密鍵が見つかりません: $key"
  }
  $env:TAURI_SIGNING_PRIVATE_KEY = $key
  $env:CARGO_TARGET_DIR = Join-Path $env:USERPROFILE '.cache/genshin-reco/target'

  npm install
  if ($LASTEXITCODE -ne 0) { throw '依存関係の準備に失敗しました。' }
  npm run check
  if ($LASTEXITCODE -ne 0) { throw '検査に失敗したため公開物を作りません。' }
  if (git status --porcelain) {
    throw '検査後に作業フォルダが変化しました。変更を確認してからやり直してください。'
  }
  npm run tauri -- build --bundles nsis --ci
  if ($LASTEXITCODE -ne 0) { throw 'デスクトップ版のビルドに失敗しました。' }

  $bundle = Join-Path $env:CARGO_TARGET_DIR 'release/bundle/nsis'
  $installers = @(Get-ChildItem -LiteralPath $bundle -File | Where-Object { $_.Name -like "*_${version}_x64-setup.exe" })
  if ($installers.Count -ne 1) {
    throw "版番号 $version のインストーラーがちょうど1個ではありません。"
  }
  $installer = $installers[0]
  $signaturePath = "$($installer.FullName).sig"
  if (-not (Test-Path -LiteralPath $signaturePath)) {
    throw '署名ファイルが見つかりません。'
  }

  $stageDir = Join-Path $env:TEMP ("genshin-reco-release-" + [guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Path $stageDir | Out-Null
  try {
    $assetName = "genshin-reco_${version}_x64-setup.exe"
    $assetPath = Join-Path $stageDir $assetName
    Copy-Item -LiteralPath $installer.FullName -Destination $assetPath
    $signature = (Get-Content -LiteralPath $signaturePath -Raw).Trim()
    $assetUrl = "https://github.com/$repo/releases/download/$tag/$assetName"
    $metadata = @{
      version = $version
      notes = "原神 聖遺物レコメンダー $version"
      pub_date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
      platforms = @{
        'windows-x86_64' = @{ url = $assetUrl; signature = $signature }
      }
    }
    $metadataPath = Join-Path $stageDir 'latest.json'
    $metadata | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $metadataPath -Encoding utf8NoBOM
    gh release create $tag $assetPath $metadataPath --repo $repo --target main --draft --title "原神 聖遺物レコメンダー $version" --notes "Windows向け正式版。ソースのコミット: $head"
    if ($LASTEXITCODE -ne 0) { throw 'GitHubの下書き作成に失敗しました。' }
    Write-Output "下書きと実機確認用インストーラーを作りました: $($installer.FullName)"
    Write-Output "確認後に実行: .\scripts\release.ps1 -Stage publish -ConfirmedTested"
  }
  finally {
    $safeTemp = [System.IO.Path]::GetFullPath($env:TEMP).TrimEnd('\') + '\'
    $safeStage = [System.IO.Path]::GetFullPath($stageDir)
    if ($safeStage.StartsWith($safeTemp, [System.StringComparison]::OrdinalIgnoreCase) -and
        [System.IO.Path]::GetFileName($safeStage) -like 'genshin-reco-release-*') {
      Remove-Item -LiteralPath $safeStage -Recurse -Force
    }
  }
}
finally {
  [Environment]::SetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY', $previousSigningKey, 'Process')
  [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $previousTargetDir, 'Process')
  Pop-Location
}
