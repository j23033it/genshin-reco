# アーキテクチャ方針

## 製品の位置づけ

本アプリはDPS計算機ではなく、4人編成の条件へ適合する候補を根拠付きで示す「編成連動ビルド推薦エンジン」である。

## 信頼境界

- Rendererは編成編集、進捗、結果、根拠、ユーザー選択の表示に限定する。
- 任意コマンド、任意ファイル、SQLite、認証情報、アクセストークン、任意URL取得をRendererへ公開しない。
- Codexは探索、抽出、構造化のみ担当する。
- Rust Trusted Coreがカタログ照合、URL検証、根拠評価、候補統合、最大81通りの編成探索、永続化を担当する。
- 外部ページ本文とCodex出力は未信頼データとして扱い、HTMLやコマンドとして実行しない。

## 採用技術

- デスクトップ: Tauri v2
- Renderer: React + TypeScript + Vite
- UI: Tailwind CSS + Base UI
- Trusted Core: Rust
- 永続化: SQLite
- Codex接続: `codex app-server`のstdio JSONL
- カタログ: Markdownをビルド時に独自Schemaへ変換し、manifestとchecksumを付けて同梱

## 仕様の優先順位

1. ユーザーの現在の依頼
2. 「要件設計レビュー」チャットの最終レビュー
3. 添付設計資料ZIP
4. 添付ゲームカタログ

最終レビューと原設計が衝突する場合は最終レビューを優先する。
