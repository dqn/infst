# infst

beatmania IIDX INFINITAS のスコアトラッカー。

Rust Edition 2024 を使用。

## プロジェクト構成

```
crates/
├── infst/        # コアライブラリ（ゲームロジック、メモリ読み取り）
└── infst-cli/    # CLI アプリケーション
```

## 開発コマンド

```bash
cargo build          # ビルド
cargo test           # テスト実行
cargo run            # CLI 実行（Windows のみ動作）
```

## CI/CD

GitHub Actions でビルド・リリースを自動化。

- **ci.yml**: PR/push 時に test, clippy, build-windows を実行
- **release.yml**: タグ push (`v*`) で Windows バイナリをビルドしリリース作成

## デバッグコマンド

INFINITAS のバージョン変更時にメモリ構造を調査するためのコマンド群。

### オフセット検索・状態確認

```bash
# オフセット検索（対話的）
infst find-offsets

# ゲーム・オフセット状態表示
infst status

# メモリ構造情報をダンプ
infst dump
```

### メモリ分析

```bash
# メモリ構造の分析（デバッグモード）
infst analyze

# 特定アドレスのメモリ構造探索
infst explore --address 0x1431B08A0

# メモリの生バイトダンプ
infst hexdump --address 0x1431B08A0 --size 256 --ascii
```

### 検索・スキャン

```bash
# メモリ検索
infst search --string "fun"              # 文字列検索（Shift-JIS）
infst search --i32 9003                  # 32bit整数検索
infst search --pattern "00 04 07 0A"     # バイトパターン検索（?? でワイルドカード）

# カスタムエントリサイズでスキャン
infst scan --entry-size 1200
```

### ユーティリティ

```bash
# アドレス間のオフセット計算
infst offset --from 0x1431B08A0 --to 0x1431B0BD0

# 楽曲エントリ構造の検証
infst validate song-entry --address 0x1431B08A0
```

## データエクスポート

全曲のプレイデータ（スコア、ランプ、ミスカウント、DJ ポイント等）をエクスポートする。

```bash
# TSV形式でファイルに出力（デフォルト）
infst export -o scores.tsv

# JSON形式でファイルに出力
infst export -o scores.json -f json

# 標準出力にTSV出力
infst export

# 標準出力にJSON出力
infst export -f json
```

### オプション

| オプション          | 説明                                   |
| ------------------- | -------------------------------------- |
| `-o, --output`      | 出力ファイルパス（省略時は標準出力）   |
| `-f, --format`      | 出力形式: `tsv`（デフォルト）/ `json`  |
| `--pid`             | プロセスID（省略時は自動検出）         |

## データ同期

メモリから直接読み取ったプレイデータを Web サービスに一括アップロードする。

```bash
# ログイン済みの場合（credentials を使用）
infst sync

# エンドポイントとトークンを明示指定
infst sync --endpoint https://infst.oidehosp.me --token <TOKEN>

# 環境変数で指定
INFST_API_ENDPOINT=https://infst.oidehosp.me INFST_API_TOKEN=<TOKEN> infst sync
```

### オプション

| オプション     | 説明                                           |
| -------------- | ---------------------------------------------- |
| `--endpoint`   | API エンドポイント URL（環境変数対応）         |
| `--token`      | API トークン（環境変数対応）                   |
| `--pid`        | プロセスID（省略時は自動検出）                 |

### 動作

1. ゲームプロセスのメモリからスコアデータを読み取り
2. 全曲 × 全難易度（SP+DP）のランプ・EXスコア・ミスカウントを収集
3. `NO PLAY` と譜面なし（total_notes == 0）をフィルタ
4. `/api/lamps/bulk` に一括 POST

API はべき等のため、何度実行しても安全。

## アーキテクチャ

### infst モジュール構成

| モジュール         | 役割                                               |
| ------------------ | -------------------------------------------------- |
| `chart/`           | 楽曲・譜面データ構造                               |
| `play/`            | ゲームプレイデータ（PlayData, Judge, Settings 等） |
| `process/`         | Windows プロセスメモリ読み取り                     |
| `score/`           | スコアデータ管理                                   |
| `session/`         | セッション管理、TSV/JSON 形式                      |
| `export/`          | データエクスポート（ExportFormat trait）           |
| `offset/`          | メモリオフセット検索・管理                         |
| `offset/searcher/` | オフセット検索のサブモジュール群                   |
| `debug/`           | メモリダンプ、スキャン、ステータス表示（要 feature） |
| `infst/`           | メインアプリケーションロジック                     |
| `prelude.rs`       | よく使う型の再エクスポート                         |
| `error.rs`         | エラー型定義                                       |

### export サブモジュール

| サブモジュール    | 役割                                     |
| ----------------- | ---------------------------------------- |
| `format.rs`       | ExportFormat trait 定義                  |
| `tsv.rs`          | TSV エクスポーター実装                   |
| `json.rs`         | JSON エクスポーター実装                  |
| `console.rs`      | コンソール出力（色付き表示）             |
| `comparison.rs`   | 自己ベスト比較ロジック                   |
| `tracker.rs`      | トラッカーデータエクスポート（TSV/JSON） |

### offset/searcher サブモジュール

| サブモジュール       | 役割                                       |
| -------------------- | ------------------------------------------ |
| `core.rs`            | OffsetSearcher 構造体と基本操作            |
| `song_list.rs`       | SongList 検索ロジック                      |
| `relative_search.rs` | 相対オフセット検索（テスト含む）           |
| `data_map.rs`        | DataMap/UnlockData 検索・検証              |
| `buffer.rs`          | バッファ管理とパターン検索ヘルパー         |
| `interactive.rs`     | 対話的オフセット検索ワークフロー           |
| `validation/`        | オフセット候補のバリデーション関数         |
| `pattern.rs`         | パターン検索ユーティリティ（memchr 使用）  |
| `constants.rs`       | 検索関連の定数                             |
| `types.rs`           | 検索結果の型定義                           |
| `utils.rs`           | ユーティリティ関数                         |
| `legacy.rs`          | レガシーシグネチャ検索（feature-gated）    |

### 主要な型

- `PlayData` - プレイ結果データ
- `Judge` - 判定データ（PGreat, Great 等）
- `SongInfo` - 楽曲メタデータ
- `Chart`, `ChartInfo` - 楽曲+難易度情報
- `UnlockData` - アンロック状態
- `Settings`, `RawSettings` - プレイ設定（生データ構造含む）
- `GameStateDetector` - ゲーム状態検出
- `ScoreMap`, `ScoreData` - ゲーム内スコアデータ
- `OffsetsCollection` - メモリオフセット集
- `OffsetSearcher`, `OffsetSearcherBuilder` - オフセット検索（Builder パターン対応）
- `SessionManager` - セッション管理
- `Infst`, `InfstConfig`, `GameData` - メインアプリケーション（設定外部化対応）
- `ExportFormat`, `TsvExporter`, `JsonExporter` - エクスポート形式（trait ベース）
- `PersonalBestComparison` - 自己ベスト比較結果

### Feature Flags

| Feature             | 説明                                               |
| ------------------- | -------------------------------------------------- |
| `debug-tools`       | debug モジュールを有効化（CLI 用、本番向けでない） |
| `legacy-signatures` | レガシーシグネチャ検索コードを有効化               |

## 参照資料

本家 C# 実装は `.agent/Reflux/` にあり。機能追加・バグ修正時に参照。

## リリース手順

1. Cargo.toml のバージョンを更新（infst, infst-cli 両方）
2. `git tag vX.Y.Z` でタグをつける
3. `git push --tags` で push

## 注意事項

- Windows 専用（INFINITAS のメモリを読み取るため）
- macOS/Linux ではビルドは通るがメモリ読み取り機能は動作しない
- Shift-JIS エンコーディング処理あり（日本語タイトル対応）
- オフセットは相対検索で検出（シグネチャ検索は無効化、後述）
- `offsets.txt` は未使用
- **このファイルは実装と同期して最新の状態を保つこと**

## オフセット検索の仕組み

### 検索戦略

オフセット検索は**相対オフセット検索**を主軸としている：

1. **SongList**: パターン検索（`"5.1.1."` バージョン文字列）でアンカーを取得
   - 期待位置 `base + 0x3180000` から検索開始（高速化のため）
2. **JudgeData**: SongList からの相対オフセット（-0x94E3C8）で検索
   - **Cross-validation**: 推論された CurrentSong 位置も検証
3. **PlaySettings**: JudgeData からの相対オフセット（-0x2ACFA8）で検索
   - **Cross-validation**: 推論された PlayData 位置も検証
4. **PlayData**: PlaySettings からの相対オフセット（+0x2A0）で検索
5. **CurrentSong**: JudgeData からの相対オフセット（+0x1E4）で検索
6. **DataMap/UnlockData**: パターン検索

### シグネチャ検索の無効化

シグネチャ（AOB）検索は **Version 2 (2026012800) で完全に機能しなくなった**ため無効化した：

| シグネチャ | Version 2 での検索結果 |
|-----------|----------------------|
| judgeData | 0件 |
| playSettings | 0件 |
| currentSong | 0件 |

コードは将来のために残しているが、デフォルトでは使用しない。

### 相対オフセットの定数値

バージョン間での相対オフセット差分（Version 1 → Version 2）：

| 関係 | Version 1 | Version 2 | 定数値 | 検索範囲 |
|------|-----------|-----------|--------|---------|
| SongList - JudgeData | 0x94E374 | 0x94E4B4 | 0x94E3C8 | ±64KB |
| JudgeData - PlaySettings | 0x2ACEE8 | 0x2ACFA8 | **0x2ACFA8** | ±512B |
| PlayData - PlaySettings | 0x2C0 | 0x2A0 | **0x2A0** | ±256B |
| CurrentSong - JudgeData | 0x1E4 | 0x1E4 | 0x1E4 | ±256B |

**注意**: 定数値は Version 2 に合わせて更新済み。検索範囲を狭めることで誤検出を防止。

### バリデーション戦略

オフセット検出の信頼性を高めるため、以下のバリデーションを実施：

1. **JudgeData**: 判定データ領域（72バイト）が all zeros または妥当な範囲内
2. **PlaySettings**: 設定値が有効範囲内 + song_select_marker チェック
3. **PlayData**: song_id が 1000-50000 の範囲内（all zeros は拒否）
4. **CurrentSong**: song_id が有効範囲内 + 2のべき乗を除外（all zeros は拒否）
5. **Cross-validation**: 関連オフセット同士の整合性を検証

### 新バージョン対応時

1. `cargo run --features debug-tools -- status` でオフセット検出状態を確認
2. 検出されたオフセットと `.agent/offsets-*.txt` の期待値を比較
3. 差分が検索範囲を超える場合は `constants.rs` の定数を更新
4. バリデーションが誤検出を起こす場合は検索範囲を狭める

### 過去の教訓

- **検索範囲は狭い方が安全**: 広い検索範囲は誤検出の原因になる
- **Cross-validation が重要**: 単体のバリデーションは弱いため、関連オフセット同士の整合性をチェック
- **all zeros の許容は危険**: 間違ったアドレスでも zeros が入っている可能性があるため、オフセット検索時は拒否する
- **定数値はバージョンごとに検証**: 新バージョン対応時は必ず実際の値と比較して更新
