# 詳細仕様

## 1. 設定

設定ファイル: `~/.config/asana-omnifocus-sync/config.toml`

```toml
workspace_gid = "1234567890"   # 対象 Asana ワークスペースの GID（必須）
omnifocus_project = "Asana"    # 取り込み先 OmniFocus プロジェクト名（省略時 "Asana"）
omnifocus_tag_root = "Asana"   # プロジェクトタグのルートタグ名（省略時 "Asana"）
tls_insecure = false           # true で TLS 証明書検証を無効化（省略時 false）
```

認証トークン: 環境変数 `ASANA_TOKEN`（必須）。設定ファイルには書かない。

設定ファイルのパスは `XDG_CONFIG_HOME` を尊重し、未設定時は `~/.config` を用いる。

### TLS / 企業プロキシ

TLS バックエンドは **native-tls（macOS では Security.framework）** を用い、システムキーチェーンの
ルート証明書を信頼する。これにより Netskope 等の TLS インターセプトプロキシが挿入する社内 CA を
（キーチェーンに導入済みであれば）**検証したまま**受け入れられる。

キーチェーンに社内 CA が無い等で検証できない場合の回避策として、`tls_insecure = true`（設定ファイル）
または CLI フラグ `--insecure` で**証明書検証を無効化**できる。検証を切るため信頼できるネットワークでのみ使う。

## 2. Asana API

- ベース URL: `https://app.asana.com/api/1.0`
- 認証: `Authorization: Bearer $ASANA_TOKEN`

### 2.1 タスク取得

- エンドポイント: `GET /tasks`
- クエリパラメータ:
  - `assignee=me`
  - `workspace=<workspace_gid>`
  - `completed_since=now` （未完了タスクのみ取得）
  - `opt_fields=name,due_on,due_at,notes,permalink_url,completed,assignee.gid,memberships.project.name,memberships.project.gid`
  - `limit=100`
- ページネーション: レスポンスの `next_page.offset` が存在する限り `offset` を付けて繰り返す。

### レスポンス（抜粋）

```json
{
  "data": [
    {
      "gid": "12345",
      "name": "タスク名",
      "due_on": "2026-06-30",
      "due_at": null,
      "notes": "詳細メモ",
      "permalink_url": "https://app.asana.com/0/0/12345",
      "completed": false,
      "memberships": [
        { "project": { "gid": "55555", "name": "プロジェクト A" } },
        { "project": { "gid": "66666", "name": "プロジェクト B" } }
      ]
    }
  ],
  "next_page": { "offset": "abcd", "path": "...", "uri": "..." }
}
```

`memberships` から各 `project.name` を取り出し、タスクの所属プロジェクト名リストとする。
`memberships` が空（どのプロジェクトにも属さない）ならプロジェクト名リストは空とする。
プロジェクト名は OmniFocus のタグ名に用いる（第 6 節）。

注: `assignee=me` + `completed_since=now` で取得されるのは「現在自分に割り当てられた未完了タスク」。
このため「担当解除されたタスク」「完了したタスク」は取得結果に現れない。同期側はこれを利用し、
OmniFocus に存在するが取得結果に無い `asana_gid` を「完了扱い」と判定する（後述）。

### 2.2 タスク完了の書き戻し

- エンドポイント: `PUT /tasks/{gid}`
- ボディ: `{ "data": { "completed": true } }`
- 用途: OmniFocus で完了したタスクの完了状態を Asana へ反映する（第 4 節の書き戻し）。
- `--dry-run` 時は呼び出さない。

書き戻しの対象は、取得結果（未完了）に現れている `gid` のうち、OmniFocus 側が完了済みのものに限る。
取得結果に現れない `gid`（Asana も既に完了）には何もしない。これにより同じ完了を毎回送らずに済む（冪等）。

## 3. OmniFocus 連携（JXA）

`/usr/bin/osascript -l JavaScript <script>` で実行する。Rust とのデータ授受は JSON。

### 3.1 dump（scripts/dump.js）

- 入力: 引数で取り込み先プロジェクト名とルートタグ名を受け取る。
- 処理: 対象プロジェクト配下のタスクのうち、note に `asana_gid:` 行を持つものを列挙する。
  各タスクの付与タグのうち、ルートタグ配下のもの（管理対象タグ）の名前を `tags` として出力する。
- 出力（stdout, JSON 配列）:

```json
[
  {
    "of_id": "OmniFocus タスクの id",
    "asana_gid": "12345",
    "name": "タスク名",
    "due": "2026-06-30T00:00:00Z 形式 or null",
    "completed": false,
    "note": "現在の note 全文",
    "tags": ["プロジェクト A", "プロジェクト B"]
  }
]
```

`tags` はルートタグ配下のタグ名のみを含む。
ルートタグ自身およびルートタグ配下にない（利用者が手動で付けた）タグは含めない。
これにより reconcile は管理対象タグだけを比較でき、他のタグを巻き込まない。

対象プロジェクトが存在しない場合は空配列 `[]` を返す（作成は apply 側で行う）。

### 3.2 apply（scripts/apply.js）

- 入力: stdin に操作 JSON を渡す。

```json
{
  "project": "Asana",
  "tag_root": "Asana",
  "operations": [
    { "kind": "create", "asana_gid": "12345", "name": "...", "due": "2026-06-30 or null", "note": "...", "tags": ["プロジェクト A"] },
    { "kind": "update", "of_id": "...", "name": "...", "due": "... or null", "note": "...", "tags": ["プロジェクト A", "プロジェクト B"] },
    { "kind": "complete", "of_id": "..." }
  ]
}
```

- 処理:
  - 取り込み先プロジェクトが無ければ作成する。作成時の種類は「単独アクション（single action list）」とする（`singletonActionHolder = true`）。
  - `create`: プロジェクト配下にタスクを作成し、name / due / note を設定し、`tags` を付与する。
  - `update`: `of_id` のタスクの name / due / note を設定し、管理対象タグを `tags` に置き換える。
  - `complete`: `of_id` のタスクを `markComplete()` する。
- タグの解決と付与:
  - ルートタグ（`tag_root`）が無ければ作成する。
  - 各 `tags` の要素について、ルートタグ配下に同名の子タグが無ければ作成し、そのタグを使う。
  - `create` ではそれらのタグをタスクに付与する。
  - `update` では、タスクから既存の管理対象タグ（ルートタグ配下のもの）をいったん外し、`tags` のタグを付与する。
    ルートタグ配下にないタグ（利用者が手動で付けたもの）は外さない。
  - `tags` が空のタスクにはルートタグのみを付与する。
- 出力（stdout, JSON）: `{ "created": n, "updated": n, "completed": n }`

### 3.3 note フォーマット

OmniFocus タスクの note は次の形式で構築する。

```
<Asana notes 本文>

---
asana_url: <permalink_url>
asana_gid: <GID>
```

`asana_gid:` 行が対応付けの正本。dump 時はこの行を正規表現 `/^asana_gid:\s*(\S+)/m` で抽出する。

## 4. 同期ルール（reconcile）

入力:
- `asana_tasks`: Asana から取得した未完了・自分担当タスクの集合（`gid` キー）。各タスクは所属プロジェクト名リストを持つ。
- `of_tasks`: dump で得た OmniFocus タスクの集合（`asana_gid` キー）。各タスクは完了フラグと管理対象タグ名リストを持つ。

出力: 適用計画 `Plan`。OmniFocus へ適用する操作 `of_ops`（create / update / complete）と、Asana へ適用する操作 `asana_ops`（complete）の二つを持つ。

突き合わせは `asana_gid` で行い、OmniFocus 側は完了・未完了で扱いを分ける。

| 条件 | 操作 |
|---|---|
| `asana_tasks` にあり、`of_tasks`（未完了）にも無く、`of_tasks`（完了済み）にも無い | `of_ops`: create |
| `asana_tasks` にあり、`of_tasks`（未完了）に存在し、name/due/note/tags に差分あり | `of_ops`: update |
| `asana_tasks` にあり、`of_tasks`（未完了）に存在し差分なし | 操作なし |
| `asana_tasks` にあり、`of_tasks`（未完了）には無いが `of_tasks`（完了済み）に有る | `asana_ops`: complete（create は出さない） |
| `of_tasks`（未完了）にあり `asana_tasks` に無い | `of_ops`: complete |
| `of_tasks`（完了済み）にあり `asana_tasks` に無い | 操作なし（双方完了済み、冪等） |

- due / note / tags の比較規則は従来どおり（due は正規化後の文字列一致、note は構築後の文字列一致、tags は順序を無視した集合比較）。`create` / `update` にはあるべきタグ集合を載せる。
- 完了の書き戻しは、OmniFocus 側が完了済みかつ Asana 側に未完了で残っている場合に限る。
  この組み合わせでは OmniFocus へのタスク再作成を抑止する。さもないと、書き戻し前のタイミングで完了タスクが OmniFocus に再作成されてしまう。
- 既に OmniFocus 側が完了済みのタスクを、Asana 取得結果に基づいて再オープンすることはしない。

## 5. CLI

```
asana-omnifocus-sync [--dry-run] [--config <path>] [--insecure]
```

- `--dry-run`: apply も Asana への書き戻しも行わず、reconcile 結果（予定操作）を表示する。
- `--config <path>`: 設定ファイルパスを上書きする。
- `--insecure`: TLS 証明書検証を無効化する（設定の `tls_insecure` より優先）。
- 終了時に `created=N updated=N completed=N asana_completed=N` のサマリを表示する。
  `asana_completed` は Asana へ完了を書き戻した件数。
