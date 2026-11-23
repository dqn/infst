# infst

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Release](https://img.shields.io/github/v/release/dqn/infst)](https://github.com/dqn/infst/releases)

beatmania IIDX INFINITAS のリアルタイムスコアトラッカー。

**Web サービス:** <https://infst.oidehosp.me> ([使い方](https://infst.oidehosp.me/guide))

## 機能

- プレイデータをリアルタイムで自動記録
- TSV/JSON 形式でスコアをエクスポート
- プレイデータを Web サービスに同期
- Web インターフェースでスコア・ランプを閲覧

## 必要条件

- Windows 専用
- beatmania IIDX INFINITAS がインストール済み

## インストール

1. [GitHub Releases](https://github.com/dqn/infst/releases) から `infst.exe` をダウンロード
2. 任意の場所に配置

## 使い方

### トラッキング

INFINITAS を起動した状態で実行：

```bash
infst
```

トラッカーの実行中、プレイが自動的に記録されます。

### 自動起動（Windows）

このリポジトリを取得したディレクトリで、Windows アカウントに自動起動を登録します：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\register-autostart.ps1 -TrackerPath "C:\Tools\infst\infst.exe"
```

登録直後とログオン時にバックグラウンドで監視を開始します。INFINITAS が起動すると、
トラッカーがフォーカスを奪わずに最小化した状態で起動します。
繰り返し起動しても、デスクトップセッションごとに監視とトラッカーを一つに保ちます。

作業ディレクトリは実行ファイルのあるディレクトリです。データの保存先を変える場合は
`-WorkingDirectory` を指定します。実行ファイルの移動後や監視スクリプトの更新後は、
同じコマンドで再登録してください。監視スクリプトとログは `%LOCALAPPDATA%\infst` に配置されます。

自動起動を解除するには：

```powershell
Stop-ScheduledTask -TaskName 'Infst Auto Start'
Unregister-ScheduledTask -TaskName 'Infst Auto Start' -Confirm:$false
```

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

### Web インターフェース

デフォルトブラウザで Web インターフェースを開く：

```bash
infst web
```

## ライセンス

[MIT License](LICENSE)
