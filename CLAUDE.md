# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 概要

Asana で自分に割り当てられた未完了タスクを OmniFocus の指定プロジェクトへ一方向同期する単一バイナリ CLI（Rust）。macOS 専用（OmniFocus 連携に JXA を使う）。

## ビルド・テスト・実行

開発は nix flake の devShell 上で行う（`.envrc` で `use flake`、direnv 有効）。

```
cargo build                     # ビルド
cargo test                      # 全テスト（reconcile のユニットテストが中心）
cargo test reconcile            # sync.rs の特定テストのみ
cargo clippy --all-targets -- --deny warnings   # CI checks と同じ lint
nix build                       # crane で再現ビルド（成果物は ./result）
nix flake check                 # clippy + test + ビルドを一括検証
```

実行には設定ファイルと環境変数が必要:

```
ASANA_TOKEN=<token> cargo run -- --dry-run    # 予定操作の表示のみ（OmniFocus 非変更）
ASANA_TOKEN=<token> cargo run                 # 実適用
cargo run -- --config <path> --insecure       # 設定パス上書き / TLS 検証無効化
```

設定ファイルは `~/.config/asana-omnifocus-sync/config.toml`（`XDG_CONFIG_HOME` 尊重）。`config.example.toml` 参照。`workspace_gid` 必須、トークンは設定ファイルに書かず `ASANA_TOKEN` で渡す。

## アーキテクチャ

実行フローは一直線で、ツール自身は永続状態を持たない（再実行安全）:

```
Asana REST (asana.rs) → reconcile (sync.rs) → apply (omnifocus.rs) → OmniFocus.app
                          ↑
        OmniFocus dump (omnifocus.rs) ──┘
```

| モジュール | 責務 |
|---|---|
| `main.rs` | CLI 引数パース、配線、`--dry-run` 分岐、サマリ表示 |
| `config.rs` | `config.toml` と `ASANA_TOKEN` の読み込み・検証 |
| `model.rs` | 共通モデル（`AsanaTask` / `OfTask` / `Operation`）、note 構築・due 正規化 |
| `asana.rs` | Asana REST クライアント（ureq）。ページネーション込みで取得 |
| `omnifocus.rs` | `osascript` 実行ラッパ。`dump()` と `apply()` |
| `sync.rs` | `reconcile(asana, of) -> Vec<Operation>`（純粋関数、テスト対象） |

### 状態は OmniFocus の note 内マーカーに持つ

対応付けの正本は OmniFocus タスクの note 末尾に書かれる `asana_gid:` 行（`build_note` で構築、dump 時に正規表現 `/^asana_gid:\s*(\S+)/m` で抽出）。DB やローカル状態ファイルは持たない。

### Asana プロジェクトは OmniFocus のタグで表現する

Asana タスクは複数プロジェクトに所属しうる（タスク↔プロジェクトが多対多）。OmniFocus のプロジェクトは 1 対 1 なので、所属プロジェクトはプロジェクトではなくタグで表す。タスク配置は単一の取り込み先プロジェクトのまま。タグはルートタグ（`omnifocus_tag_root`、既定 "Asana"）配下にプロジェクト名でネストする。`AsanaTask.projects`（所属プロジェクト名）と `OfTask.tags`（ルートタグ配下の管理対象タグ名）を集合比較し、差分があれば update する。同期が触れるのはルートタグ配下のタグのみで、利用者の手動タグは保持する。リネームは別タグ扱い、空タグの削除はしない。

### 完了判定のしくみ

Asana 取得は `assignee=me` + `completed_since=now` なので「現在自分担当の未完了タスク」のみ返る。よって OmniFocus（未完了）に存在するが Asana 取得結果に無い `asana_gid` は「完了/担当解除」とみなし `complete` する。OmniFocus 側で既に完了済みのタスクは突き合わせ対象から除外し、再オープンしない。

### reconcile は純粋関数

`sync::reconcile` は I/O を持たず入力スライスから `Vec<Operation>` を返すのみ。create/update/complete の判定はここに集約され `cargo test` で副作用なく検証する。同期ロジックを変えるときはまず sync.rs のテストを追加すること。

### JXA スクリプトは埋め込み

`scripts/dump.js` / `scripts/apply.js` は `include_str!` でバイナリに埋め込み、実行時に一時ファイルへ書き出して `osascript -l JavaScript` で実行する（外部ファイル依存なし）。Rust ⇔ JXA のデータ授受は JSON（dump は引数でプロジェクト名→stdout に JSON 配列、apply は stdin に操作 JSON→stdout にサマリ）。

### TLS / 社内プロキシ

HTTPS は ureq + **native-tls**（macOS の Security.framework）でシステムキーチェーンのルート証明書を信頼する。Netskope 等の TLS インターセプト CA がキーチェーンにあれば検証したまま通る。検証できない場合のみ `tls_insecure = true` / `--insecure` で無効化（信頼できるネットワーク限定）。

## 開発フロー（このリポジトリの規約）

- 仕様駆動開発。実装前に `docs/`（use-cases.md / requirements.md / spec.md / design.md）を更新する。
- TDD（t-wada 流）: テスト追加 → RED 確認 → 最小実装 → GREEN → リファクタリング。実装より先にテストを書く。
- バージョン管理は jj を使用（git は jj で不可能な場合のみ）。コード変更前に `jj commit`。push はユーザー要求時のみ。
- 仕様の正本は `docs/spec.md`。同期ルール・note フォーマット・API パラメータの詳細はここを参照・更新する。
