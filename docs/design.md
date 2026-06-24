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
    projects: Vec<String>,      // 所属プロジェクト名（タグに対応。空ならルートタグのみ）
}

struct OfTask {
    of_id: String,
    asana_gid: String,
    name: String,
    due: Option<String>,
    completed: bool,
    note: String,
    tags: Vec<String>,          // 管理対象タグ（ルートタグ配下の子タグ名）
}

enum Operation {           // OmniFocus へ適用する操作
    Create { asana_gid: String, name: String, due: Option<String>, note: String, tags: Vec<String> },
    Update { of_id: String, name: String, due: Option<String>, note: String, tags: Vec<String> },
    Complete { of_id: String },
}

enum AsanaOp {             // Asana へ適用する操作（完了の書き戻し）
    Complete { gid: String, name: String },   // name は表示用
}

struct Plan {              // reconcile の出力。適用先ごとに分ける
    of_ops: Vec<Operation>,
    asana_ops: Vec<AsanaOp>,
}
```

`Operation`（OmniFocus 行き）と `AsanaOp`（Asana 行き）を別の型に分けるのは、適用経路が異なるためである。
`Operation` は apply.js へ JSON で渡して `osascript` で適用し、`AsanaOp` は `asana.rs` の REST クライアントで適用する。
両者を一つの列挙にまとめると、apply.js が解さない操作が混ざりうる。

`build_note(notes, url, gid)` と `normalize_due(due_on, due_at)` を model.rs に置き、
asana.rs と sync.rs の双方から使う（マッピングの一貫性確保）。

## Asana プロジェクトをタグで表現する

OmniFocus のタスクは 1 つのプロジェクトにしか属せないが、Asana のタスクは複数プロジェクトに同時所属しうる。
この非対称を避けるため、Asana プロジェクトを OmniFocus の**プロジェクトではなくタグ**で表現する。
タスクの配置は従来どおり単一の取り込み先プロジェクトのままにし、所属プロジェクトはタグの多重付与で表す。

タグはルートタグ（既定 `Asana`）の配下にプロジェクト名でネストする。
ネストにより、利用者がコンテキストや場所に使う一般のタグと名前空間が分かれ、同期が管理するタグを判別できる。
管理対象はルートタグ配下のタグに限り、それ以外のタグには触れない。

この設計には次の利点がある。
タスクの配置と dump の走査範囲が単一プロジェクトのままなので、複数プロジェクトに散らした場合に生じる「別プロジェクトのタスクを Asana 不在と誤判定して完了させる」リスクが無い。
所属変更も OmniFocus 上のタスク移動を伴わず、タグの差し替えだけで済む。

同定はタグ名（プロジェクト名）で行う。
Asana 側のリネームは別タグとして扱い、旧タグは残す（破壊的操作を避けるため削除しない）。
所属解除や同名プロジェクトの衝突も同様に、タグ名の一致だけを根拠に扱う。

### reconcile でのタグ差分

`reconcile` は name / due / note に加えてタグ集合も比較する。
Asana の所属プロジェクト名リストと OmniFocus の管理対象タグ名リストを、順序を無視した集合として突き合わせ、差があれば `Update` を生成する。
`Create` / `Update` の操作には、あるべきタグ集合（所属プロジェクト名リスト）を載せる。
比較対象は管理対象タグだけなので、利用者の手動タグは差分計算に影響しない。

### apply.js でのタグ操作

`apply.js` はルートタグを取得（無ければ作成）し、その配下に各プロジェクトタグを取得（無ければ作成）する。
`create` ではタスク作成後にそれらのタグを付与する。
`update` ではタスクから既存の管理対象タグを外してから新しいタグ集合を付与し、ルートタグ配下にないタグは温存する。

JXA でのネストタグ作成（ルートタグの子として子タグを追加する手順）は実装着手時に確認する。

## 完了の書き戻し（OmniFocus → Asana）

作成・更新・タグ付けは Asana を正本とする一方向同期だが、完了だけは双方向に伝える。
Asana で完了したものは OmniFocus を完了し、OmniFocus で完了したものは Asana を完了する。

書き戻しの判定は `reconcile` の中で行う。
`OfTask` は完了フラグを保持しており（dump.js が `completed` を出力する）、`reconcile` は OmniFocus 側を完了・未完了に分けて突き合わせる。
ある `asana_gid` が「OmniFocus で完了済み」かつ「Asana 取得結果（未完了）に残っている」とき、`AsanaOp::Complete` を出す。

このケースで OmniFocus への `Create` を抑止するのが要点である。
従来の一方向同期では、完了済みの `OfTask` を未完了の突き合わせ対象から外していたため、同じ `gid` が Asana に未完了で残っていると `Create` が出て OmniFocus にタスクが再作成された。
書き戻しでは、この再作成を `AsanaOp::Complete` に置き換える。
Asana を完了にすれば、次回以降の取得結果に当該 `gid` は現れず、再作成も書き戻しも起きない（収束する）。

OmniFocus 側で完了済みかつ Asana 取得結果にも現れない `gid`（双方が既に完了）には何もしない。
Asana を毎回完了し直さないための冪等性の担保である。

書き戻し先が `osascript` ではなく Asana REST なので、`main` は `Plan.of_ops` を `omnifocus::apply` に、`Plan.asana_ops` を `AsanaClient::complete_task` に渡す。
`--dry-run` ではどちらの適用も行わない。

なお完了の取得には窓がある。
OmniFocus 側で完了タスクが「クリーンアップ」でアーカイブされると dump に現れなくなり、書き戻せない。
これは最善努力（best-effort）の挙動とする。

## reconcile の純粋性

`reconcile` は I/O を持たず、入力スライスから `Plan`（`of_ops` と `asana_ops`）を返すだけにする。
これにより作成/更新/完了/書き戻しの各ケースを `cargo test` で副作用なく検証できる。
完了済みの `OfTask` も入力に含めて渡す。書き戻し判定に完了フラグが要るためで、`reconcile` 内で完了・未完了に振り分ける。

## エラーハンドリング

`anyhow` で集約。致命的エラー（設定不足、トークン未設定、osascript 失敗、API エラー）は
非ゼロ終了＋メッセージ。部分的失敗（個別タスクの適用失敗）は apply.js 内で握りつぶさず、
できる範囲で続行しつつ失敗件数を返す方針は MVP では採らず、まず全体を素直に失敗させる。

## nix / ビルド

- `flake.nix`: crane で `cargo build` を再現実行。devShell に `rustc`, `cargo`, `rust-analyzer`, `clippy`。
- `.envrc`: `use flake`（direnv）。
- 成果物は単一バイナリ。launchd 化（後続）でもそのまま使える。
