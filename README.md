# 原神 聖遺物レコメンダー

原神 Ver.7.0向けの、根拠付き・編成連動ビルド推薦デスクトップアプリです。

Codex App Serverは攻略情報の調査・抽出・構造化に限定して利用し、入力検証、根拠照合、候補統合、編成内の組み合わせ選択、保存はRust側の決定論的ロジックで行います。

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

## 現在の開発段階

Gate 0として、TauriのRust側からCodex App Serverを起動し、認証状態、構造化出力、イベント順序、キャンセル、異常終了を検証しています。本格的な推薦UIはGate 0通過後に実装します。
