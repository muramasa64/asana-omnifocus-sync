# 詳細仕様

## 1. 設定

設定ファイル: `~/.config/asana-omnifocus-sync/config.toml`

```toml
workspace_gid = "1234567890"   # 対象 Asana ワークスペースの GID（必須）
omnifocus_project = "Asana"    # 取り込み先 OmniFocus プロジェクト名（省略時 "Asana"）
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
- エンドポイント: `GET /tasks`
- クエリパラメータ:
  - `assignee=me`
  - `workspace=<workspace_gid>`
  - `completed_since=now` （未完了タスクのみ取得）
  - `opt_fields=name,due_on,due_at,notes,permalink_url,completed,assignee.gid`
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
      "completed": false
    }
  ],
  "next_page": { "offset": "abcd", "path": "...", "uri": "..." }
}
```

注: `assignee=me` + `completed_since=now` で取得されるのは「現在自分に割り当てられた未完了タスク」。
このため「担当解除されたタスク」「完了したタスク」は取得結果に現れない。同期側はこれを利用し、
OmniFocus に存在するが取得結果に無い `asana_gid` を「完了扱い」と判定する（後述）。

## 3. OmniFocus 連携（JXA）

`/usr/bin/osascript -l JavaScript <script>` で実行する。Rust とのデータ授受は JSON。

### 3.1 dump（scripts/dump.js）

- 入力: 引数または環境変数で取り込み先プロジェクト名を受け取る。
- 処理: 対象プロジェクト配下のタスクのうち、note に `asana_gid:` 行を持つものを列挙する。
- 出力（stdout, JSON 配列）:

```json
[
  {
    "of_id": "OmniFocus タスクの id",
    "asana_gid": "12345",
    "name": "タスク名",
    "due": "2026-06-30T00:00:00Z 形式 or null",
    "completed": false,
    "note": "現在の note 全文"
  }
]
```

対象プロジェクトが存在しない場合は空配列 `[]` を返す（作成は apply 側で行う）。

### 3.2 apply（scripts/apply.js）

- 入力: stdin に操作 JSON を渡す。

```json
{
  "project": "Asana",
  "operations": [
    { "kind": "create", "asana_gid": "12345", "name": "...", "due": "2026-06-30 or null", "note": "..." },
    { "kind": "update", "of_id": "...", "name": "...", "due": "... or null", "note": "..." },
    { "kind": "complete", "of_id": "..." }
  ]
}
```

- 処理:
  - 取り込み先プロジェクトが無ければ作成する。
  - `create`: プロジェクト配下にタスクを作成し、name / due / note を設定する。
  - `update`: `of_id` のタスクの name / due / note を設定する。
  - `complete`: `of_id` のタスクを `markComplete()` する。
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
- `asana_tasks`: Asana から取得した未完了・自分担当タスクの集合（`gid` キー）。
- `of_tasks`: dump で得た OmniFocus タスクの集合（`asana_gid` キー）。未完了のもののみ突き合わせ対象。

出力: 操作リスト（create / update / complete）。

| 条件 | 操作 |
|---|---|
| `asana_tasks` にあり `of_tasks`（未完了）に無い | `create` |
| 双方に存在し、name または due または note に差分あり | `update` |
| 双方に存在し差分なし | 操作なし |
| `of_tasks`（未完了）にあり `asana_tasks` に無い | `complete` |

- due の比較は日付（または日時）の正規化後の文字列一致で行う。
- note の比較は `asana_url` / `asana_gid` を含む構築後の文字列同士で行う（マーカーの揺れを避ける）。
- 既に OmniFocus 側が完了済みのタスクは突き合わせ対象から除外する（再オープンしない）。

## 5. CLI

```
asana-omnifocus-sync [--dry-run] [--config <path>] [--insecure]
```

- `--dry-run`: apply を行わず、reconcile 結果（予定操作）を表示する。
- `--config <path>`: 設定ファイルパスを上書きする。
- `--insecure`: TLS 証明書検証を無効化する（設定の `tls_insecure` より優先）。
- 終了時に `created=N updated=N completed=N` のサマリを表示する。
