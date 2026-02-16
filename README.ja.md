# infst

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Release](https://img.shields.io/github/v/release/dqn/reflux-rs)](https://github.com/dqn/reflux-rs/releases)

beatmania IIDX INFINITAS のリアルタイムスコアトラッカー。

## 機能

- プレイデータをリアルタイムで自動記録
- TSV/JSON 形式でスコアをエクスポート
- プレイデータを Web サービスに直接同期

## 必要条件

- Windows 専用
- beatmania IIDX INFINITAS がインストール済み

## インストール

1. [GitHub Releases](https://github.com/dqn/reflux-rs/releases) から `infst.exe` をダウンロード
2. 任意の場所に配置

## 使い方

### トラッキング

INFINITAS を起動した状態で実行：

```bash
infst
```

トラッカーの実行中、プレイが自動的に記録されます。

### データエクスポート

全プレイデータ（スコア、ランプ、ミスカウント、DJ ポイント等）をエクスポート：

```bash
# TSV形式でエクスポート（デフォルト）
infst export -o scores.tsv

# JSON形式でエクスポート
infst export -o scores.json -f json

# 標準出力に出力
infst export
```

#### オプション

| オプション | 説明 |
|-----------|------|
| `-o, --output` | 出力ファイルパス（省略時は標準出力） |
| `-f, --format` | 出力形式: `tsv`（デフォルト）/ `json` |

### データ同期

ゲームメモリから直接プレイデータを Web サービスに同期：

```bash
# 初回のみログイン
infst login

# プレイデータを同期
infst sync
```

#### オプション

| オプション | 説明 |
|-----------|------|
| `--endpoint` | API エンドポイント URL（環境変数: `INFST_API_ENDPOINT`） |
| `--token` | API トークン（環境変数: `INFST_API_TOKEN`） |
| `--pid` | プロセスID（省略時は自動検出） |

## ライセンス

[MIT License](LICENSE)
