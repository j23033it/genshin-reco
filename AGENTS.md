# このプロジェクトの作業ルール

## 古いビルド情報を使わない

- Rust または Tauri のコマンドを実行する前に、`git rev-parse --show-toplevel` で現在の作業場所を確認し、同じ PowerShell で `./scripts/use-cargo-target.ps1` を実行する。`npm run check` と `npm run tauri dev` も対象。
- ビルド先は作業場所の絶対パスから決める。プロジェクトを移動したり別の worktree を使ったりした場合、以前の `src-tauri/target` や共通キャッシュを再利用しない。
- エラーに現在の作業場所と異なる絶対パスが出たら、まず古い生成物の参照を疑う。ソースの修正や依存更新を始める前に、実際の作業場所・`CARGO_TARGET_DIR`・エラー内のパスを照合し、新しいビルド先で再試行する。
- 古いビルド先は原因確認だけに使い、自動削除しない。削除が必要な場合は対象の絶対パスと内容を確認し、ユーザーのファイルや未追跡の変更を巻き込まない。

## 正式版の公開

- 更新と公開の作業は、このリポジトリ限定の `genshin-reco-release` スキル（`.agents/skills/genshin-reco-release/SKILL.md`）に従う。
