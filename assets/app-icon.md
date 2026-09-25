# デスクトップ用アイコン

- 元画像: `app-icon.png`（組み込みのImageGenで生成・調整）
- 意匠: 原神のアプリと一目でわかるように、パイモンの白髪・浮いた冠・星の髪飾りを大きく描いた顔アップ。濃紺の背景の外側は透過。
- 配布用画像: `src-tauri/icons/`。Tauriの `icon` コマンドで元画像から変換し、既存のデスクトップ用ファイル一式を差し替え。
- `src-tauri/tauri.conf.json` の `bundle.icon` が配布用画像を参照する。Windowsの実行ファイルと次回のインストーラーに組み込まれ、デスクトップのショートカットも実行ファイルのアイコンを使う。
- この変更では版番号の更新と公開は行わない。

## 生成時のプロンプト

```text
Use case: stylized-concept
Asset type: A single finished desktop app icon for a Genshin Impact artifact recommendation companion app.
Primary request: Make the icon instantly recognizable as Genshin Impact by depicting PAIMON, the actual recognizable Genshin Impact character, in a charming polished chibi anime portrait. This replaces an overly generic flower icon. Recognizability is the main priority.
Subject: Paimon's face and fluffy short white hair dominate the icon, with her distinctive small dark navy four-point star hair ornament at the viewer's right side, blue-violet eyes, warm smile, peach-pink floating crown halo clearly visible above her head, and a little of her white and peach outfit with dark starry navy scarf/cape visible below. Preserve Paimon's recognizable canonical character design and hairstyle. Front-facing, happy, confident expression. Exactly one character.
Composition: Square desktop icon, centered close-up head and shoulders, face large enough to read at 32 and 48 pixels. Hair and halo fully inside the image, not clipped. Paimon occupies roughly 85 percent of a deep midnight navy rounded-square tile. Small 3 percent transparent outer margin. Halo visibly separated from white hair against the dark backdrop. Minimal navy backdrop gives strong silhouette contrast.
Style/medium: Beautiful clean anime game UI illustration, crisp strong outlines, simplified cel shading, large readable features, warm peach accents, white hair, restrained cyan and gold star accents in the backdrop. Flat front-facing app icon, not a physical object or mockup. No excessive texture or fine detail.
Constraints: One square PNG with actual alpha transparency outside the rounded tile. No letters, no title, no words, no watermark, no grids, no alternatives, no decorative frame, no extra characters, no realistic human skin, no generic flower emblem. This must look unmistakably like Paimon from Genshin Impact, not a generic white-haired mascot.
```

## 仕上げのプロンプト

```text
Use case: precise-object-edit
Edit target: The supplied Paimon desktop icon. Keep exactly this recognizable Paimon character, face, hair, expression, peach halo crown, navy star hair clip, colors and illustration style.
Change only framing and edge finish: scale the complete composition down slightly so the entire crown tip has a clean 5 percent canvas margin above it, and all sides of the rounded navy tile have at least 4 percent clear canvas margin. Clean the lower edge: the character is cleanly clipped to the smoothly rounded navy tile there; remove the stray white flecks and all pixels below the tile. No ragged outlines. Keep the large face prominent. All colored artwork and the tile must be fully opaque (alpha 255) except antialiasing at silhouette edges, and outside the icon must be truly transparent (alpha 0). Keep a square image. Do not redesign anything, add any text, watermark or new objects.
```
