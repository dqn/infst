# オフセット自動検出（現行実装）

## 概要

reflux-rs の CLI は、**組み込みシグネチャ（AOB スキャン）**で主要オフセットを検出し、
残りは **データパターン検索**で補います。`offsets.txt` は CLI では読み込まれません。

## CLI の挙動

- オフセットが無効な場合に署名ベース検出を実行（初回接続時は必ず実行）
- `offsets.txt` は未使用、CLI 引数も未実装
- 検出に失敗するとエラー終了
- バージョン検出に成功した場合、検出結果の `version` に反映

## 検出フェーズ（`search_all_with_signatures`）

1. **SongList（署名）**
   - 候補を取得し、**曲数が 1000 以上**かで検証
2. **JudgeData（署名）**
   - `STATE_MARKER` が 0〜100 に収まるかで検証
3. **PlaySettings（署名）**
   - スタイル/ゲージ/アシスト等が有効範囲にあるかで検証
4. **PlayData（署名）**
   - 曲 ID / 難易度 / EX / ミス数が妥当かを検証（全ゼロは許容）
5. **CurrentSong（署名）**
   - 曲 ID / 難易度の範囲と「2 の冪」を除外するヒューリスティック
6. **DataMap / UnlockData（パターン）**
   - DataMap: base から探索、失敗時は SongList 近傍を探索
   - UnlockData: 固定パターンの **最後の一致**を採用

## 検証ロジック（`validate_signature_offsets`）

以下の**相対オフセット**に近いことを確認します（値は `constants.rs` で定義）。

- `judgeData - playSettings ≈ 0x2ACEE8`（±0x2000）
- `songList - judgeData ≈ 0x94E3C8`（±0x10000）
- `playData - playSettings ≈ 0x2C0`（±0x100）
  - 2025122400 以降: 0x2C0
  - それ以前: 0x2B0
- `currentSong - judgeData ≈ 0x160`（±0x100）

## 既知の制約

- DataMap / UnlockData はデータパターン依存のため誤検出リスクが残る
- CLI は外部 `offset-signatures.json` を読み込まない（`load_signatures` はライブラリ側に存在）
- インタラクティブ検索はライブラリにあるが、CLI からは未配線

## 関連ファイル

- `crates/reflux-core/src/offset/searcher/mod.rs`
- `crates/reflux-core/src/offset/searcher/constants.rs`
- `crates/reflux-core/src/offset/signature.rs`
- `crates/reflux-core/src/offset/loader.rs`
