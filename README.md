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

1. 調べたい4人を自然文で入力する
2. Codexが不足している名前や任意条件だけを確認する
3. 4人が揃ったら調査を開始し、進捗を会話内に表示する
4. 調査に成功した編成だけを保存する
5. 保存結果をキャラクター・武器・聖遺物・目標ステータスのカードで表示する
6. 同じ会話を開き直し、条件変更後に再調査する
