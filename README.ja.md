# Reflux-RS

[Reflux](https://github.com/olji/Reflux) の Rust 再実装。beatmania IIDX INFINITAS 用スコアトラッカー。

[English version](README.md)

## 機能

- **メモリ読み取り**: INFINITAS プロセスから直接ゲームデータを読み取り
- **スコア記録**: 判定、スコア、クリアランプを含むプレイ結果を記録
- **署名ベースのオフセット検出**: AOB スキャン + 残りはパターン検索
- **ローカル保存**: TSV セッション（デフォルト）、tracker.db、tracker.tsv、unlockdb
- **任意の出力**: JSON セッション、latest-*.txt、楽曲情報ファイル（設定で有効化）
- **リモート同期（任意）**: カスタムサーバ API（設定で有効化）
- **サポートファイル**: `encodingfixes.txt` / `customtypes.txt`（任意）

## 本家 Reflux との機能比較

| カテゴリ | 機能 | 本家 (C#) | 本実装 (Rust) | 備考 |
|----------|------|-----------|---------------|------|
| **コア** | メモリ読み取り | ✅ | ✅ | |
| | ゲーム状態検出 | ✅ | ✅ | |
| | オフセット自動検索 | ✅ | ✅ | 署名 (AOB) + パターン検索 |
| | バージョン検出 | ✅ | ✅ | |
| **データ** | プレイデータ | ✅ | ✅ | スコア、ランプ、グレード |
| | 判定データ | ✅ | ✅ | P1/P2、Fast/Slow |
| | 設定 | ✅ | ✅ | スタイル、ゲージ、アシスト、H-RAN |
| | アンロック追跡 | ✅ | ✅ | |
| **保存** | TSV セッション | ✅ | ✅ | |
| | JSON セッション | ✅ | ✅ | 設定で有効化 |
| | トラッカー DB | ✅ | ✅ | ベストスコア |
| | アンロック DB | ✅ | ✅ | |
| **リモート** | サーバー同期 | ✅ | ✅ | |
| | ファイル更新 | ✅ | ⚠️ | API はあるが CLI から未呼び出し |
| | Kamaitachi | ⚠️ | ⚠️ | 楽曲検索ヘルパーのみ（ライブラリ） |
| **配信** | playstate/marquee | ✅ | ✅ | |
| | latest-*.txt | ✅ | ✅ | 設定で有効化 |
| | 楽曲情報ファイル | ✅ | ✅ | 設定で有効化 |
| **設定** | INI 設定 | ✅ | ⚠️ | パーサのみ、CLI はデフォルト固定 |
| **追加** | GitHub 更新チェック | ❌ | ❌ | 未配線 |

✅ = 実装済み, ⚠️ = 部分実装, ❌ = 未実装

## 動作要件

- Windows（ReadProcessMemory API を使用）
- Rust 1.85+（Edition 2024）
- beatmania IIDX INFINITAS

## インストール

### ソースからビルド

```bash
git clone https://github.com/dqn/reflux-rs.git
cd reflux-rs
cargo build --release
```

バイナリは `target/release/reflux.exe` に生成されます。

## 使い方

```bash
# デフォルト設定で実行
reflux

# ヘルプを表示
reflux --help
```

現在の CLI について:
- CLI 引数は未定義で、常にデフォルト設定で動きます。
- `config.ini` は **読み込まれません**（パーサは core にあります）。
- オフセットは組み込みシグネチャで解決し、`offsets.txt` は読み込みません。

CLI が使うファイル:
- トラッカー DB: `tracker.db`
- トラッカー出力: `tracker.tsv`（選曲画面と切断時に出力）
- アンロック DB: `unlockdb`
- セッション: `sessions/Session_YYYY_MM_DD_HH_MM_SS.tsv`
- 任意のサポートファイル: `encodingfixes.txt`, `customtypes.txt`
- 任意のデバッグ出力: `songs.tsv`（`debug.outputdb = true`）

## 設定（パーサはあるが CLI では未使用）

`Config::load` で読み込めますが、現状の CLI は `Config::default()` 固定です。

```ini
[Update]
updatefiles = true
updateserver = https://raw.githubusercontent.com/olji/Reflux/master/Reflux

[Record]
saveremote = false
savelocal = true
savejson = false
savelatestjson = false
savelatesttxt = false

[RemoteRecord]
serveraddress =
apikey = your-api-key

[LocalRecord]
songinfo = false
chartdetails = false
resultdetails = false
judge = false
settings = false

[Livestream]
playstate = false
marquee = false
fullsonginfo = false
marqueeidletext = INFINITAS

[Debug]
outputdb = false
```

## オフセット

CLI は組み込みシグネチャでオフセットを検出し、`offsets.txt` は読み込みません。
core ライブラリでは `load_offsets`/`save_offsets` が利用できます。

`offsets.txt` の形式（先頭行はバージョン）:

```
P2D:J:B:A:2025010100
songList = 0x12345678
dataMap = 0x12345678
judgeData = 0x12345678
playData = 0x12345678
playSettings = 0x12345678
unlockData = 0x12345678
currentSong = 0x12345678
```

CLI は起動時にオフセットが無効な場合、署名ベースで検出を試みます。

## プロジェクト構成

```
reflux-rs/
├── Cargo.toml              # ワークスペース設定
├── crates/
│   ├── reflux-core/        # コアライブラリ
│   │   └── src/
│   │       ├── config/     # INI 設定パーサー
│   │       ├── game/       # ゲームデータ構造
│   │       ├── memory/     # Windows API ラッパー
│   │       ├── network/    # HTTP クライアント、Kamaitachi API
│   │       ├── offset/     # オフセット管理
│   │       ├── storage/    # ローカル永続化
│   │       ├── stream/     # OBS 配信出力
│   │       ├── reflux/     # メイントラッカーロジック
│   │       └── error.rs    # エラー型
│   │
│   └── reflux-cli/         # CLI アプリケーション
│       └── src/main.rs
```

## 出力ファイル

### セッションファイル

プレイデータは `sessions/Session_YYYY_MM_DD_HH_MM_SS.tsv` に保存されます。
`savejson = true` の場合、`sessions/Session_YYYY_MM_DD_HH_MM_SS.json` も出力されます。

### 配信用ファイル（OBS 用、設定で有効化）

| ファイル | 説明 |
|---------|------|
| `playstate.txt` | 現在の状態: `menu`、`play`、`off` |
| `marquee.txt` | 現在の楽曲タイトルまたはステータス |
| `latest.json` | 最新のプレイ結果（post form JSON） |
| `latest.txt` | 最新のプレイ結果（3 行: タイトル/グレード/ランプ） |
| `latest-grade.txt` | 最新のグレード（AAA、AA など） |
| `latest-lamp.txt` | 最新のクリアランプ（展開名） |
| `latest-difficulty.txt` | 最新の難易度短縮名 |
| `latest-difficulty-color.txt` | 難易度カラーコード |
| `latest-titleenglish.txt` | 英題 |
| `title.txt` | 曲名 |
| `artist.txt` | アーティスト |
| `englishtitle.txt` | 英題 |
| `genre.txt` | ジャンル |
| `folder.txt` | フォルダ番号 |
| `level.txt` | レベル |

## 開発

```bash
# ビルド
cargo build

# テスト実行
cargo test

# ログ付きで実行
RUST_LOG=reflux=debug cargo run

# コード品質チェック
cargo clippy
```

## ライセンス

MIT License - 詳細は [LICENSE](LICENSE) を参照。

## クレジット

- オリジナルの [Reflux](https://github.com/olji/Reflux) by olji
- スコアトラッキングプラットフォーム [Kamaitachi/Tachi](https://github.com/zkrising/Tachi)
