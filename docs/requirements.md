# 要件

## 機能要件

- FR-1: Asana REST API から「自分に割り当て」かつ「未完了」のタスクを取得する。
- FR-2: 取得対象ワークスペースは設定で指定する。
- FR-3: 取得したタスクを OmniFocus の取り込み先プロジェクト（既定名 `Asana`）へ反映する。
  プロジェクトが存在しなければ「単独アクション」として作成する。
- FR-4: OmniFocus タスクと Asana タスクの対応付けは、OmniFocus タスクの note に埋め込んだ
  `asana_gid: <GID>` マーカーで行う。これにより再実行時の重複を防ぐ。
- FR-5: 同期は以下の差分操作を行う。作成・更新・タグ付けは Asana を正本とし一方向（Asana → OmniFocus）で反映する。完了のみ双方向に伝える。
  - 新規（OF に未存在）: OmniFocus タスクを作成する。
  - 更新（双方に存在し差分あり）: name / due / note / タグを更新する。
  - 完了（Asana 側が完了 or 担当解除）: OmniFocus タスクを完了状態にする。
  - 完了の書き戻し（OF 側が完了済みで Asana 側が未完了）: Asana タスクを完了状態にする（FR-10）。
- FR-6: フィールドマッピング
  - Asana `name` → OmniFocus タスク名
  - Asana `due_at`（無ければ `due_on`）→ OmniFocus due date（無ければ未設定）
  - Asana `notes` + `permalink_url` + `asana_gid:` マーカー → OmniFocus note
  - Asana 所属プロジェクト名 → OmniFocus タグ（FR-9）
- FR-7: `--dry-run` 指定時は OmniFocus も Asana も変更せず、予定操作を表示する。
- FR-8: 実行後に作成/更新/完了の件数サマリを表示する。
- FR-9: Asana タスクの所属プロジェクトを OmniFocus のタグとして反映する。
  - タグはルートタグ（既定名 `Asana`、設定で変更可）の配下に、プロジェクト名でネストする。
  - Asana のタスクは複数プロジェクトに所属しうる。所属する各プロジェクトに対応するタグをすべて付与する。
  - どのプロジェクトにも所属しないタスクには、ルートタグのみを付与する。
  - 同期で管理するのはルートタグ配下のタグに限る。タスクに付いた他のタグ（利用者が手動で付けたもの）は保持する。
  - 更新時、タスクの管理対象タグは現在の Asana 所属に合わせて全置換する（所属解除されたプロジェクトのタグは外す）。
  - ルートタグおよびプロジェクトタグが存在しなければ作成する。空になったタグの削除は行わない。
  - プロジェクト名の同定はタグ名で行う。Asana 側のリネームは別タグとして扱う（旧タグは残る）。
- FR-10: OmniFocus で完了したタスクの完了状態を Asana へ書き戻す。
  - 対象は、OmniFocus 側で完了済み（`completed=true`）かつ Asana 取得結果（未完了）に当該 `asana_gid` が残っているタスク。
  - 当該 Asana タスクを `PATCH /tasks/{gid}`（`completed=true`）で完了にする。
  - このとき OmniFocus 側のタスク再作成は抑止する（さもないと完了タスクが OmniFocus に復活する）。
  - 書き戻すのは完了のみ。OmniFocus の破棄・保留・defer は対象外。
  - OmniFocus 側で既に完了し、Asana 取得結果にも現れない（既に Asana も完了）タスクは、何もしない（冪等）。
  - `--dry-run` 時は Asana を変更せず、書き戻し予定のみ表示する。

## 非機能要件

- NFR-1: 環境構築・ビルドは nix flake で再現可能であること（crane 利用、単一バイナリ生成）。
- NFR-2: 実装言語は Rust。外部依存は最小限に保つ。
- NFR-3: Asana PAT はソースや設定ファイルに直書きせず、環境変数 `ASANA_TOKEN` から読む。
- NFR-4: OmniFocus 連携は `osascript -l JavaScript`（JXA）経由。`/usr/bin/osascript` を絶対パスで呼ぶ。
- NFR-5: JXA 呼び出しは取得（dump）と適用（apply）の 2 回に集約し、osascript 起動コストを抑える。
- NFR-6: 差分計算ロジックは副作用のない純粋関数として実装し、ユニットテストで検証可能にする。

## 制約・前提

- C-1: macOS（Apple Silicon）上で動作する。OmniFocus が起動していること。
- C-2: OmniFocus Pro の AppleScript/JXA 連携が利用可能であること。
- C-3: ネットワーク経由で Asana API に到達できること。
- C-4: 完了の書き戻し（FR-10）には書き込み権限を持つ PAT が必要。Asana タスクの完了は当該タスクのフォロワーや関連オートメーションに波及しうる。
