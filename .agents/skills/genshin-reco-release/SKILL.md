---
name: genshin-reco-release
description: このリポジトリのWindows版アプリをGitHub経由で更新・公開するときに、版番号、UIの開発版確認、署名付きビルド、正式公開、公開後の保存データ確認を扱う。公開手順の見直しにも使う。
---

# 原神 聖遺物レコメンダーの正式版公開

このリポジトリ専用。ソースは非公開の `j23033it/genshin-reco`、配布先は公開の `j23033it/genshin-reco-releases`。公開物は Windows 用 NSIS インストーラーと `latest.json`。タグは `app-v<版番号>`、配布名は `genshin-reco_<版番号>_x64-setup.exe`。

## 公開の条件

- ユーザーが今回のアプリ更新を GitHub 経由で公開するよう明示的に依頼していない限り、下書き作成を含む GitHub Releases や更新配信の操作をしない。準備や画面確認の成功を公開依頼と解釈しない。
- 開発中はアプリをインストールしない。UI を変更した場合は、正式版と異なる識別子で `tauri dev` を起動し、確認できる画面と操作を公開前に確かめる。正式版の保存データは開発版へコピーしない。保存データが必要な動作とアプリ内更新は、公開後にユーザーが正式版を更新して確認する。公開前に検証済みと表現しない。
- 正式公開前には対象コミットの全体検査、署名付きビルド、下書きの配布物と SHA256 の照合を通す。開発版で確認できる UI が失敗または未確認なら下書きのまま止め、状況を報告する。インストーラー単体での導入はこの方針では検証せず、保存データとアプリ内更新は公開後に確認する。
- 署名用の秘密鍵は `$env:USERPROFILE\.config\genshin-reco\updater.key`。内容を表示・記録・コミット・アップロードしない。安全な別の場所へバックアップを保つ。紛失すると既存アプリに新版を配れない。Windows のコード署名証明書とは別物で、現時点では付けていないため初回導入時に SmartScreen の警告が出る可能性がある。

## UI 変更の確認

- UI 変更があるときだけ `tauri dev` で確認する。Tauri の `--config` に渡す開発用 JSON はリポジトリ外に置き、以後再利用する。例えば `{"productName":"原神 聖遺物レコメンダー 開発版","identifier":"jp.taiki.genshinreco.dev"}` とし、正式版の設定ファイルは変更しない。開発版の保存先が正式版と分かれることを確認する。既存の正式版データを開発版にコピーしない。必要なら開発版の保存先だけに仮データを作る。
- リポジトリ直下で `git rev-parse --show-toplevel` を確認し、同じ PowerShell で `./scripts/use-cargo-target.ps1` を実行してから `npm run tauri -- dev --config <開発用 JSON のパス>` で起動する。開発版で確認できる画面と変更した操作を確認して終了する。開発版で確認できる UI に失敗した場合は公開しない。実際の保存データがないと確認できない範囲は公開後へ回し、未確認と報告する。
- 正式版の保存データが必要な一覧・結果・条件変更や更新動作は、公開後にユーザーが正式版を更新してから確認する。開発版の仮データで見た画面と、正式版の保存データでの動作確認を区別して報告する。

## 準備

1. UI 変更があれば上記の方法で確認する。公開する版番号はユーザー指定を優先し、指定がなければ現版のパッチ番号を1つ上げる。以下をリポジトリ直下で実行して5ファイルを更新する。変更が既にある場合も勝手に破棄しない。失敗したら差分を確認して止める。

```powershell
$ErrorActionPreference = 'Stop'
$requestedVersion = $null # ユーザーが版番号を指定した場合のみ、ここへ文字列を設定
$oldVersion = (Get-Content package.json -Raw | ConvertFrom-Json).version
$current = [version]$oldVersion
$nextVersion = if ($requestedVersion) { $requestedVersion } else {
  '{0}.{1}.{2}' -f $current.Major, $current.Minor, ($current.Build + 1)
}
if ($nextVersion -notmatch '^\d+\.\d+\.\d+$' -or [version]$nextVersion -le $current) {
  throw '新しい正式版の版番号を確認してください。'
}
$rules = @(
  @{ Path = 'src-tauri/Cargo.toml'; Pattern = '(?m)^(version = ")[^"]+(")$' },
  @{ Path = 'src-tauri/Cargo.lock'; Pattern = '(?m)(^\[\[package\]\]\r?\nname = "genshin-reco"\r?\nversion = ")[^"]+(")' },
  @{ Path = 'src-tauri/tauri.conf.json'; Pattern = '(?m)^(\s*"version": ")[^"]+(".*)$' }
)
$edits = foreach ($rule in $rules) {
  $path = (Resolve-Path -LiteralPath $rule.Path).Path
  $raw = [IO.File]::ReadAllText($path)
  $matches = [regex]::Matches($raw, $rule.Pattern)
  if ($matches.Count -ne 1) { throw "版番号の位置が不明です: $path" }
  $match = $matches[0]
  $start = $match.Groups[1].Index + $match.Groups[1].Length
  $end = $match.Groups[2].Index
  if ($raw.Substring($start, $end - $start) -ne $oldVersion) {
    throw "元の版番号が一致しません: $path"
  }
  @{ Path = $path; Content = $raw.Substring(0, $start) + $nextVersion + $raw.Substring($end) }
}
npm version $nextVersion --no-git-tag-version --ignore-scripts
if ($LASTEXITCODE -ne 0) { throw 'npm の版番号更新に失敗しました。' }
foreach ($edit in $edits) {
  [IO.File]::WriteAllText($edit.Path, $edit.Content, [Text.UTF8Encoding]::new($false))
}
$package = Get-Content package.json -Raw | ConvertFrom-Json
$lock = Get-Content package-lock.json -Raw | ConvertFrom-Json -AsHashtable
if ($package.version -ne $nextVersion -or $lock['version'] -ne $nextVersion -or
    $lock['packages']['']['version'] -ne $nextVersion) {
  throw 'Node 側の版番号が一致しません。'
}
git diff --check
if ($LASTEXITCODE -ne 0) { throw '版番号の差分を確認してください。' }
```

Rust/Tauri 側の3か所は上の置換前検査で同じ旧版と確認している。実行後に差分を読み、版番号以外が変わっていないことを確かめる。
2. 必要な変更をコミットして `main` に統合し、`origin/main` に送る。GitHub の全体検査を一度通す。下のコードが対象 SHA の成功、作業フォルダ、版番号、既存タグの有無を再確認する。認証・通信エラーを「タグなし」と扱わず、公開準備中に全体検査を繰り返さない。

## 下書き作成

今回の公開依頼がある場合だけ、リポジトリ直下から次の PowerShell を**一つの呼び出し**で実行する。事前の手動ビルドや全体検査の再実行はしない。エラーが出たら下書きの有無を確認し、同じタグで作成処理を繰り返さない。

```powershell
$ErrorActionPreference = 'Stop'
$sourceRepo = 'j23033it/genshin-reco'
$releaseRepo = 'j23033it/genshin-reco-releases'
$root = (git rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0 -or
    [IO.Path]::GetFullPath((Resolve-Path .).Path) -ne [IO.Path]::GetFullPath($root)) {
  throw '作業場所を確認してください。'
}
if ((git branch --show-current).Trim() -ne 'main' -or (git status --porcelain)) {
  throw 'main の作業フォルダをきれいにしてください。'
}
$config = Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json
$package = Get-Content package.json -Raw | ConvertFrom-Json
$lock = Get-Content package-lock.json -Raw | ConvertFrom-Json -AsHashtable
$manifestVersion = [regex]::Match((Get-Content src-tauri/Cargo.toml -Raw), '(?m)^version = "([^"]+)"$').Groups[1].Value
$cargoLockVersion = [regex]::Match((Get-Content src-tauri/Cargo.lock -Raw), '(?m)^\[\[package\]\]\r?\nname = "genshin-reco"\r?\nversion = "([^"]+)"').Groups[1].Value
$version = $config.version
$otherVersions = @($package.version, $lock['version'], $lock['packages']['']['version'], $manifestVersion, $cargoLockVersion)
if ($version -notmatch '^\d+\.\d+\.\d+$' -or
    @($otherVersions | Where-Object { $_ -ne $version }).Count -gt 0) {
  throw '5ファイルの版番号を確認してください。'
}
$tag = "app-v$version"
$head = (git rev-parse HEAD).Trim()
git fetch --quiet origin main
if ($LASTEXITCODE -ne 0 -or (git rev-parse origin/main).Trim() -ne $head) {
  throw 'ローカル main と origin/main が一致しません。'
}
$runJson = gh run list --repo $sourceRepo --workflow check.yml --branch main --commit $head --event push --json headSha,status,conclusion,url --limit 1
if ($LASTEXITCODE -ne 0) { throw 'GitHub の全体検査を取得できません。' }
$runs = @(ConvertFrom-Json -InputObject ($runJson -join [Environment]::NewLine))
if ($runs.Count -ne 1 -or $runs[0].headSha -ne $head -or
    $runs[0].status -ne 'completed' -or $runs[0].conclusion -ne 'success') {
  throw 'このコミットの GitHub 全体検査が成功していません。'
}
$listJson = gh release list --repo $releaseRepo --limit 1000 --json tagName
if ($LASTEXITCODE -ne 0) { throw '配布先のタグを確認できません。' }
$releases = @(ConvertFrom-Json -InputObject ($listJson -join [Environment]::NewLine))
if (@($releases | Where-Object tagName -eq $tag).Count -gt 0) {
  throw "$tag は既に存在します。"
}
$oldSigning = $env:TAURI_SIGNING_PRIVATE_KEY
$oldTarget = $env:CARGO_TARGET_DIR
$stageDir = $null
try {
  & ./scripts/use-cargo-target.ps1
  if (-not [IO.Path]::IsPathFullyQualified($env:CARGO_TARGET_DIR)) {
    throw 'Cargo ビルド先が絶対パスではありません。'
  }
  $key = Join-Path $env:USERPROFILE '.config/genshin-reco/updater.key'
  if (-not (Test-Path -LiteralPath $key)) { throw '署名鍵が見つかりません。' }
  $env:TAURI_SIGNING_PRIVATE_KEY = $key
  npm ci
  if ($LASTEXITCODE -ne 0 -or (git status --porcelain)) { throw '依存関係の準備に失敗しました。' }
  npm run tauri -- build --bundles nsis --ci
  if ($LASTEXITCODE -ne 0 -or (git status --porcelain)) { throw '署名付きビルドに失敗しました。' }
  $bundle = Join-Path $env:CARGO_TARGET_DIR 'release/bundle/nsis'
  $installers = @(Get-ChildItem -LiteralPath $bundle -File |
    Where-Object Name -like "*_$($version)_x64-setup.exe")
  if ($installers.Count -ne 1) { throw '該当版のインストーラーが1個ではありません。' }
  $signaturePath = "$($installers[0].FullName).sig"
  if (-not (Test-Path -LiteralPath $signaturePath)) { throw '署名ファイルがありません。' }
  $signature = (Get-Content -LiteralPath $signaturePath -Raw).Trim()
  if (-not $signature) { throw '署名が空です。' }
  $stageDir = Join-Path $env:TEMP ('genshin-reco-release-' + [guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Path $stageDir | Out-Null
  $assetName = "genshin-reco_$($version)_x64-setup.exe"
  $assetPath = Join-Path $stageDir $assetName
  $metadataPath = Join-Path $stageDir 'latest.json'
  Copy-Item -LiteralPath $installers[0].FullName -Destination $assetPath
  $assetUrl = "https://github.com/$releaseRepo/releases/download/$tag/$assetName"
  $metadata = @{
    version = $version
    notes = "原神 聖遺物レコメンダー $version"
    pub_date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = @{ 'windows-x86_64' = @{ url = $assetUrl; signature = $signature } }
  }
  $metadata | ConvertTo-Json -Depth 5 |
    Set-Content -LiteralPath $metadataPath -Encoding utf8NoBOM
  $localHash = (Get-FileHash -LiteralPath $assetPath -Algorithm SHA256).Hash.ToLowerInvariant()
  $metadataHash = (Get-FileHash -LiteralPath $metadataPath -Algorithm SHA256).Hash.ToLowerInvariant()
  gh release create $tag $assetPath $metadataPath --repo $releaseRepo --target main --draft --title "原神 聖遺物レコメンダー $version" --notes "Windows向け正式版。ソースのコミット: $head"
  if ($LASTEXITCODE -ne 0) { throw '公開下書きの作成に失敗しました。' }
  $releaseJson = gh release view $tag --repo $releaseRepo --json isDraft,body,assets
  if ($LASTEXITCODE -ne 0) { throw '作成した下書きを確認できません。' }
  $release = $releaseJson | ConvertFrom-Json
  $exe = @($release.assets | Where-Object name -eq $assetName)
  $json = @($release.assets | Where-Object name -eq 'latest.json')
  if (-not $release.isDraft -or $release.body -notlike "*ソースのコミット: $head*" -or
      $exe.Count -ne 1 -or $json.Count -ne 1 -or
      $exe[0].digest -ne "sha256:$localHash" -or
      $json[0].digest -ne "sha256:$metadataHash") {
    throw '下書きの内容またはハッシュが一致しません。公開せず確認してください。'
  }
  Write-Output "下書き: https://github.com/$releaseRepo/releases/tag/$tag"
  Write-Output "下書きとの照合用: $($installers[0].FullName)"
  Write-Output "SHA256: $localHash"
  Write-Output "ソース: $head / 全体検査: $($runs[0].url)"
}
finally {
  $env:TAURI_SIGNING_PRIVATE_KEY = $oldSigning
  $env:CARGO_TARGET_DIR = $oldTarget
  if ($stageDir) { Write-Output "確認後に片付ける一時フォルダ: $stageDir" }
}
```

`gh release view` の SHA256 はアップロード後の配布物を確認するために使う。表示されない場合は、下書きのファイルを認証付きでダウンロードしてハッシュを照合し、成功するまで正式公開へ進まない。秘密鍵の内容やソースを配布先へ送らない。一時ファイルの削除は配布確認後に、表示された絶対パスと内容を確認して別の操作で行う。

## 正式公開

UI 変更があれば開発版での画面確認結果を確認する。保存データを使う動作とアプリ内更新は公開後の正式版で確認するため、公開時点では未確認として報告する。インストーラー単体での導入は未検証として扱う。

次の変数を**今回の依頼と変更内容・画面確認結果に基づいて**設定し、公開直前の照合と公開を一つの PowerShell で行う。UI 変更がないと確認できた場合だけ `$uiChanged` を `$false` にする。UI 変更がある場合は `tauri dev` で確認できる範囲に成功し、実データ待ちの範囲を記録したときだけ `$confirmedUiCheck` を `$true` にする。

```powershell
$ErrorActionPreference = 'Stop'
$confirmedReleaseRequest = $false # 今回の公開依頼が明示されている場合だけ変更
$uiChanged = $true                # UI 変更がないと確認できた場合だけ false に変更
$confirmedUiCheck = $false        # UI 変更時に開発版で可能な画面を確認し、実データ待ちを記録した場合だけ変更
if (-not $confirmedReleaseRequest -or ($uiChanged -and -not $confirmedUiCheck)) {
  throw '公開依頼と、UI 変更時の画面確認が必要です。'
}
$repo = 'j23033it/genshin-reco-releases'
$root = (git rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0 -or
    [IO.Path]::GetFullPath((Resolve-Path .).Path) -ne [IO.Path]::GetFullPath($root)) {
  throw '作業場所を確認してください。'
}
if ((git branch --show-current).Trim() -ne 'main' -or (git status --porcelain)) {
  throw 'main の作業フォルダをきれいにしてください。'
}
$head = (git rev-parse HEAD).Trim()
git fetch --quiet origin main
if ($LASTEXITCODE -ne 0 -or (git rev-parse origin/main).Trim() -ne $head) {
  throw 'ソースのコミットが変わっています。'
}
$version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
$tag = "app-v$version"
$assetName = "genshin-reco_$($version)_x64-setup.exe"
$releaseJson = gh release view $tag --repo $repo --json isDraft,body,assets
if ($LASTEXITCODE -ne 0) { throw '下書きを取得できません。' }
$release = $releaseJson | ConvertFrom-Json
$exe = @($release.assets | Where-Object name -eq $assetName)
$json = @($release.assets | Where-Object name -eq 'latest.json')
if (-not $release.isDraft -or $release.body -notlike "*ソースのコミット: $head*" -or
    $exe.Count -ne 1 -or $json.Count -ne 1 -or @($release.assets).Count -ne 2) {
  throw '下書きのソースまたは配布物が一致しません。'
}
$oldTarget = $env:CARGO_TARGET_DIR
$tempJson = Join-Path $env:TEMP ('genshin-reco-latest-' + [guid]::NewGuid().ToString('N') + '.json')
try {
  & ./scripts/use-cargo-target.ps1
  $bundle = Join-Path $env:CARGO_TARGET_DIR 'release/bundle/nsis'
  $installers = @(Get-ChildItem -LiteralPath $bundle -File |
    Where-Object Name -like "*_$($version)_x64-setup.exe")
  if ($installers.Count -ne 1) { throw '下書きに対応するインストーラーを確認できません。' }
  $signaturePath = "$($installers[0].FullName).sig"
  if (-not (Test-Path -LiteralPath $signaturePath)) { throw '署名がありません。' }
  $signature = (Get-Content -LiteralPath $signaturePath -Raw).Trim()
  $hash = (Get-FileHash -LiteralPath $installers[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($exe[0].digest -ne "sha256:$hash") { throw '下書きのインストーラーが手元のビルドと違います。' }
  gh release download $tag --repo $repo --pattern latest.json --output $tempJson
  if ($LASTEXITCODE -ne 0) { throw '下書きの更新情報を取得できません。' }
  $metadataHash = (Get-FileHash -LiteralPath $tempJson -Algorithm SHA256).Hash.ToLowerInvariant()
  $metadata = Get-Content -LiteralPath $tempJson -Raw | ConvertFrom-Json
  $expectedUrl = "https://github.com/$repo/releases/download/$tag/$assetName"
  if ($json[0].digest -ne "sha256:$metadataHash" -or
      $metadata.version -ne $version -or
      $metadata.platforms.'windows-x86_64'.url -ne $expectedUrl -or
      $metadata.platforms.'windows-x86_64'.signature -ne $signature) {
    throw '下書きの更新情報がビルドと一致しません。'
  }
  gh release edit $tag --repo $repo --draft=false --latest
  if ($LASTEXITCODE -ne 0) { throw '正式公開に失敗しました。' }
  Write-Output "正式公開: https://github.com/$repo/releases/tag/$tag"
  Write-Output "SHA256: $hash"
}
finally {
  $env:CARGO_TARGET_DIR = $oldTarget
  if (Test-Path -LiteralPath $tempJson) { Write-Output "確認後に片付ける一時ファイル: $tempJson" }
}
```

公開後は認証情報を送らずに次を実行する。GitHub の `latest` が今回の版を指すこと、更新情報とインストーラーのダウンロードが成功し、公開物の SHA256 が下書きと同じであることを確認する。

```powershell
$ErrorActionPreference = 'Stop'
$repo = 'j23033it/genshin-reco-releases'
$version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
$tag = "app-v$version"
$assetName = "genshin-reco_$($version)_x64-setup.exe"
$releaseJson = gh release view $tag --repo $repo --json isDraft,assets
if ($LASTEXITCODE -ne 0) { throw '公開版を取得できません。' }
$release = $releaseJson | ConvertFrom-Json
$exe = @($release.assets | Where-Object name -eq $assetName)
$json = @($release.assets | Where-Object name -eq 'latest.json')
if ($release.isDraft -or $exe.Count -ne 1 -or $json.Count -ne 1) {
  throw '公開版の配布物が揃っていません。'
}
$tempDir = Join-Path $env:TEMP ('genshin-reco-public-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tempDir | Out-Null
try {
  $metadataPath = Join-Path $tempDir 'latest.json'
  $installerPath = Join-Path $tempDir $assetName
  Invoke-WebRequest -Uri "https://github.com/$repo/releases/latest/download/latest.json" -OutFile $metadataPath
  $metadataHash = (Get-FileHash -LiteralPath $metadataPath -Algorithm SHA256).Hash.ToLowerInvariant()
  $metadata = Get-Content -LiteralPath $metadataPath -Raw | ConvertFrom-Json
  $expectedUrl = "https://github.com/$repo/releases/download/$tag/$assetName"
  if ($json[0].digest -ne "sha256:$metadataHash" -or
      $metadata.version -ne $version -or
      $metadata.platforms.'windows-x86_64'.url -ne $expectedUrl -or
      -not $metadata.platforms.'windows-x86_64'.signature) {
    throw '公開された更新情報が一致しません。'
  }
  Invoke-WebRequest -Uri $expectedUrl -OutFile $installerPath
  $hash = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($exe[0].digest -ne "sha256:$hash") { throw '公開インストーラーのハッシュが一致しません。' }
  Write-Output "認証なしの取得に成功: https://github.com/$repo/releases/tag/$tag"
}
finally {
  Write-Output "確認後に片付ける一時フォルダ: $tempDir"
}
```

表示された一時ファイルとフォルダは、絶対パスと内容を確認して別の操作で片付ける。アプリ内の更新はユーザーが行う。公開後、ユーザーが旧版から正式版に更新し、保存済みデータを使う一覧・結果・条件変更と更新動作を確認する。確認結果が届くまではこれらを未確認と報告する。公開済み版へ問題が見つかったら状態を報告し、別の版番号で修正する。公開済み配布物を上書きしない。

公開前の確認に失敗した段階以降は進めない。公開したか、下書きで止まったか、版番号・ソース SHA・検査結果・UI 確認結果・公開後に残る確認・配布 URL を短く報告する。古いビルド先やバックアップは自動削除しない。
