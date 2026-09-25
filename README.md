# 原神 聖遺物レコメンダー

知りたい4人だけを会話から調査し、結果を画像付きカードとして保存する原神向けデスクトップアプリです。

最初からゲーム情報をすべて同梱しません。Codex App Serverが会話から4人と任意条件を整理し、必要な情報だけをWeb調査します。Rust側は入力、根拠URL、目標ステータス、保存内容を検証し、会話・完成編成・再利用可能な調査断片をSQLiteへ保存します。

## 開発環境

- Tauri v2
- React 19 / TypeScript / Vite
- Tailwind CSS v4 / Base UI
- Rust stable（MSVC）
- Vitest

## コマンド

```powershell
npm install
npm run dev
npm run test
npm run build
npm run tauri dev
npm run check
```

## 基本の流れ

1. 調べたい4人を自然文で入力する。名前が足りなければCodexが聞き返す
2. 4人の凸と武器を画面で選ぶ。未確定なら「指定なし」のまま進める
3. 調査の進捗を会話内に表示する
4. 調査に成功した編成だけを保存する
5. 保存結果をキャラクター・武器・聖遺物・目標ステータスのカードで表示する
6. 同じ会話を開き直し、条件変更後に再調査する

## デスクトップ版と試運転版

- 日々の画面確認は `npm run dev` の `http://localhost:1420/?demo=1` で行う。固定のデモデータだけを使い、実際の調査・保存・更新は行わない。
- Codex接続や保存を含む動作確認は `npm run tauri dev` で行う。この開発版から正式版の更新はしない。
- 正式版はWindows用インストーラーで一度だけ導入する。デスクトップにアプリのショートカットができ、以後はアプリ内の「更新を確認」から署名済みの新しい版を導入する。
- ソースは非公開のGitHubリポジトリで管理する。公開するのは別リポジトリ `j23033it/genshin-reco-releases` の署名済みインストーラーと `latest.json` だけ。更新の通信先はそこだけで、デモや開発版は更新しない。

## 正式版の更新手順（管理者向け）

1. 変更をデモと `tauri dev` で確認し、`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` の版番号を同じ新しい値にする。
2. 検査が通った変更を `main` へ統合して非公開の `origin/main` に送る。GitHubの「変更の確認」が成功したことを見る。
3. `main` の作業フォルダをきれいにして `./scripts/release.ps1 -Stage prepare` を実行する。検査・署名・ビルド・公開下書きの作成まで自動で行い、版番号に一致するインストーラーだけを選ぶ。
4. 出力されたインストーラーを実機で一度確認する。問題がなければ `./scripts/release.ps1 -Stage publish -ConfirmedTested` で正式公開する。公開後、既存アプリの「更新を確認」から入ることを確認する。

署名用の秘密鍵は `C:\Users\taiki\.config\genshin-reco\updater.key` にあり、GitHubやこのリポジトリへ入れない。安全な場所へ別途バックアップすること。紛失すると既存アプリへ新しい版を配れなくなる。Windowsの配布用コード署名証明書は別物で、現時点では付けていないため、初回導入時にSmartScreenの警告が出る可能性がある。
