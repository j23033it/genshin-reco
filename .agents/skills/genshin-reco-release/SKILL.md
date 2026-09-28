---
name: genshin-reco-release
description: このリポジトリのWindows版アプリをGitHub経由で更新・公開するときに、版番号、検査、署名付きビルド、下書きでの実機確認、正式公開を扱う。公開手順の見直しにも使う。
---

# 原神 聖遺物レコメンダーの正式版公開

このリポジトリ専用。ソースは非公開の `j23033it/genshin-reco`、配布先は公開の `j23033it/genshin-reco-releases`。公開物は Windows 用 NSIS インストーラーと `latest.json`。タグは `app-v<版番号>`、配布名は `genshin-reco_<版番号>_x64-setup.exe`。

## 公開の条件

- ユーザーが今回のアプリ更新を GitHub 経由で公開するよう明示的に依頼していない限り、下書き作成を含む GitHub Releases や更新配信の操作をしない。準備や実機確認の成功を公開依頼と解釈しない。
- 正式公開は、今回作った下書きのインストーラーを実機で確認して成功した場合だけ行う。失敗や未確認なら下書きのまま止め、状況を報告する。
- 署名用の秘密鍵は `$env:USERPROFILE\.config\genshin-reco\updater.key`。内容を表示・記録・コミット・アップロードしない。安全な別の場所へバックアップを保つ。紛失すると既存アプリに新版を配れない。Windows のコード署名証明書とは別物で、現時点では付けていないため初回導入時に SmartScreen の警告が出る可能性がある。

## 準備

1. 変更した動作だけを手元で確認し、版番号を `package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock` のアプリ本体項目、`src-tauri/tauri.conf.json` で一致させる。既存タグと同じ版番号は使わない。
2. 必要な変更をコミットして `main` に統合し、`origin/main` に送る。公開作業は `main` のきれいな作業フォルダから行う。`git fetch origin main` の後、`HEAD` と `origin/main` が同じ SHA であることを確認する。
3. `gh run list --repo j23033it/genshin-reco --workflow check.yml --branch main --commit <HEADのSHA> --event push --json headSha,status,conclusion,url --limit 1` で、その SHA の GitHub 全体検査が `completed` / `success` と確認できるまで進めない。公開準備中に `npm run check` を繰り返さない。
4. `gh release view app-v<版番号> --repo j23033it/genshin-reco-releases` でタグが存在しないことを確かめる。認証・通信エラーを「タグなし」と扱わない。存在する版の公開物を上書きしない。

## 下書き作成

1. `git rev-parse --show-toplevel` で作業場所を確認する。同じ PowerShell で `./scripts/use-cargo-target.ps1` を実行し、`CARGO_TARGET_DIR` が現在の作業場所に対応した絶対パスであることを確かめる。ビルドまで同じ PowerShell 呼び出しで行い、移動前や別 worktree の生成物を使わない。
2. 秘密鍵ファイルの存在だけを確認し、その PowerShell のプロセス環境変数 `TAURI_SIGNING_PRIVATE_KEY` にその**パス**を設定する。既存の `TAURI_SIGNING_PRIVATE_KEY` と `CARGO_TARGET_DIR` は `try/finally` で復元する。
3. `npm ci`、続けて `npm run tauri -- build --bundles nsis --ci` を一度だけ実行する。Tauri が `npm run build` も実行するため、事前の手動ビルドを重ねない。失敗したら止める。各実行後に未コミット変更と未追跡ファイルがないことを確認する。
4. `$env:CARGO_TARGET_DIR\release\bundle\nsis` から、版番号に一致する NSIS インストーラーがちょうど1個あり、その `.sig` ファイルもあることを確認する。署名は `.sig` の文字列を使う。
5. 一時フォルダにインストーラーを `genshin-reco_<版番号>_x64-setup.exe` としてコピーし、BOM なし UTF-8 の `latest.json` を作る。内容は `version`、`notes`、UTC の `pub_date`、`platforms.windows-x86_64.url` と `signature`。URL は `https://github.com/j23033it/genshin-reco-releases/releases/download/app-v<版番号>/genshin-reco_<版番号>_x64-setup.exe`、署名は手順4の `.sig` と完全一致させる。
6. `gh release create app-v<版番号> <コピーしたインストーラー> <latest.json> --repo j23033it/genshin-reco-releases --target main --draft --title "原神 聖遺物レコメンダー <版番号>" --notes "Windows向け正式版。ソースのコミット: <HEADのSHA>"` で**下書き**を作る。公開用の配布先へソースや鍵を含めない。一時ファイルは、内容と保存先を確認してから片付ける。

## 実機確認と正式公開

1. 下書きの版番号、インストーラー名、インストーラーのハッシュ、`.sig` と `latest.json` の署名・URLを照合する。下書きのインストーラーを実機に導入し、起動と保存済みデータの読み込みを確認する。必要なバックアップは残す。
2. 公開直前に `main` の作業フォルダがきれいで `HEAD = origin/main` のままか再確認する。下書きの説明に同じソース SHA があり、必要な2つの配布物が揃っていることを `gh release view` で確認する。違えば止める。
3. ユーザーの今回の公開依頼と手順1の実機確認が揃ったら、`gh release edit app-v<版番号> --repo j23033it/genshin-reco-releases --draft=false --latest` で正式公開する。
4. 公開後、認証情報なしで `latest.json` とインストーラーを取得できること、公開した `latest.json` の版番号・URL・署名とインストーラーのハッシュが下書きで確認したものと一致することを調べる。旧版アプリの「更新を確認」に新版が表示されることも確認する。

失敗した段階以降は進めない。公開したか、下書きで止まったか、版番号・ソース SHA・検査結果・実機確認結果・配布 URL を短く報告する。古いビルド先やバックアップは自動削除しない。
