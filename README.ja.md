# reflux-rs

beatmania IIDX INFINITAS のスコアトラッカー。本家 [Reflux](https://github.com/olji/Reflux)（C#）の Rust 移植版です。

## 機能

- ベストスコアの自動追跡（tracker.db）
- セッションごとのプレイ記録（TSV/JSON）
- OBS 配信向けテキストファイル出力

## 動作環境

- Windows 専用
- beatmania IIDX INFINITAS が動作している環境

## インストール

[Releases](https://github.com/dqn/reflux-rs/releases) から最新版をダウンロードしてください。

## 使い方

1. beatmania IIDX INFINITAS を起動
2. `reflux.exe` を実行
3. ゲームを自動検出してトラッキングを開始

### ログレベルの変更

`RUST_LOG` 環境変数でログレベルを変更できます：

```
RUST_LOG=debug reflux.exe
```

## 出力ファイル

### ベストスコア

| ファイル | 説明 |
|----------|------|
| `tracker.db` | ベストスコア（独自形式、自動保存） |
| `tracker.tsv` | ベストスコア（TSV 形式でエクスポート） |

### セッション記録

セッションファイルはカレントディレクトリに作成されます：

| ファイル | 説明 |
|----------|------|
| `Session_YYYY_MM_DD_HH_MM_SS.tsv` | セッションのプレイ記録 |
| `Session_YYYY_MM_DD_HH_MM_SS.json` | JSON 形式のプレイ記録 |

## OBS 連携

OBS オーバーレイ用に以下のテキストファイルが出力されます：

### プレイ状態

| ファイル | 説明 |
|----------|------|
| `playstate.txt` | 現在の状態：`menu`、`play`、`off` |

### 選曲中の楽曲情報

| ファイル | 説明 |
|----------|------|
| `title.txt` | 楽曲タイトル（日本語） |
| `englishtitle.txt` | 楽曲タイトル（英語） |
| `artist.txt` | アーティスト名 |
| `genre.txt` | ジャンル |
| `level.txt` | レベル |
| `folder.txt` | フォルダ名 |
| `currentsong.txt` | 結合形式：`タイトル [難易度レベル]` |

### 最新リザルト

| ファイル | 説明 |
|----------|------|
| `latest.txt` | タイトル、グレード、ランプ（複数行） |
| `latest-grade.txt` | グレード（例：AAA, AA, A） |
| `latest-lamp.txt` | クリアランプ（例：FULL COMBO, HARD CLEAR） |
| `latest-difficulty.txt` | 難易度（例：SPA, SPH） |
| `latest-difficulty-color.txt` | 難易度のカラーコード |
| `latest-titleenglish.txt` | 英語タイトル |
| `latest.json` | JSON 形式のフルリザルト |

## サポートファイル（任意）

以下のファイルを `reflux.exe` と同じディレクトリに配置してください：

### encodingfixes.txt

楽曲タイトルの文字化けを修正します。形式：

```
誤った文字列	正しい文字列
```

### customtypes.txt

楽曲の分類をカスタマイズします。形式：

```
楽曲ID	タイプ名
```

## ライセンス

MIT
