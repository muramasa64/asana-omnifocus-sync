# 設計ドキュメント

## 全体像

単一バイナリの CLI。実行フローは「Asana 取得 → OmniFocus 現状取得（dump）→ 差分計算（reconcile）
→ OmniFocus 適用（apply）→ サマリ表示」の一直線。状態は OmniFocus タスクの note 内マーカーに持たせ、
ツール自身は永続状態を持たない（再実行安全）。

```
            ┌─────────┐      ┌──────────┐      ┌──────────┐
 Asana REST │ asana.rs │──→──│ sync.rs  │──→──│omnifocus │──→ OmniFocus.app
 (ureq)     └─────────┘     │reconcile │     │ apply.js │   (JXA)
                            └──────────┘     └──────────┘
                                  ↑
            OmniFocus.app ──→ omnifocus dump.js（現状取得）
```

## モジュール構成

| モジュール | 責務 |
|---|---|
| `main.rs` | CLI 引数パース、各モジュールの配線、サマリ表示、`--dry-run` 分岐 |
| `config.rs` | `config.toml` と `ASANA_TOKEN` の読み込み・検証 |
| `model.rs` | 共通モデル（`AsanaTask`, `OfTask`, `Operation`）、note 構築・due 正規化 |
| `asana.rs` | Asana REST クライアント（ureq）。ページネーション込みでタスク取得 |
| `omnifocus.rs` | `osascript` 実行ラッパ。`dump()` と `apply()` を提供 |
| `sync.rs` | `reconcile(asana, of) -> Vec<Operation>`（純粋関数）。ユニットテスト対象 |

`scripts/dump.js` / `scripts/apply.js` は JXA。バイナリへ `include_str!` で埋め込み、
実行時に `osascript -l JavaScript -` の stdin へ渡す（外部ファイル依存を無くし配布を単純化）。

## データモデル（model.rs）

```rust
struct AsanaTask {
    gid: String,
    name: String,
    due: Option<String>,        // 正規化済み（日付 or 日時）
    notes: String,
    permalink_url: String,
}

struct OfTask {
    of_id: String,
    asana_gid: String,
    name: String,
    due: Option<String>,
    completed: bool,
    note: String,
}

enum Operation {
    Create { asana_gid: String, name: String, due: Option<String>, note: String },
    Update { of_id: String, name: String, due: Option<String>, note: String },
    Complete { of_id: String },
}
```

`build_note(notes, url, gid)` と `normalize_due(due_on, due_at)` を model.rs に置き、
asana.rs と sync.rs の双方から使う（マッピングの一貫性確保）。

## reconcile の純粋性

`reconcile` は I/O を持たず、入力スライスから `Vec<Operation>` を返すだけにする。
これにより新規/更新/完了の 3 ケースを `cargo test` で副作用なく検証できる。
OmniFocus 側で完了済みの `OfTask` は呼び出し前に除外する（main 側 or reconcile 冒頭でフィルタ）。

## エラーハンドリング

`anyhow` で集約。致命的エラー（設定不足、トークン未設定、osascript 失敗、API エラー）は
非ゼロ終了＋メッセージ。部分的失敗（個別タスクの適用失敗）は apply.js 内で握りつぶさず、
できる範囲で続行しつつ失敗件数を返す方針は MVP では採らず、まず全体を素直に失敗させる。

## nix / ビルド

- `flake.nix`: crane で `cargo build` を再現実行。devShell に `rustc`, `cargo`, `rust-analyzer`, `clippy`。
- `.envrc`: `use flake`（direnv）。
- 成果物は単一バイナリ。launchd 化（後続）でもそのまま使える。
