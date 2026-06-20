# 要件

## 機能要件

- FR-1: Asana REST API から「自分に割り当て」かつ「未完了」のタスクを取得する。
- FR-2: 取得対象ワークスペースは設定で指定する。
- FR-3: 取得したタスクを OmniFocus の取り込み先プロジェクト（既定名 `Asana`）へ反映する。
  プロジェクトが存在しなければ作成する。
- FR-4: OmniFocus タスクと Asana タスクの対応付けは、OmniFocus タスクの note に埋め込んだ
  `asana_gid: <GID>` マーカーで行う。これにより再実行時の重複を防ぐ。
- FR-5: 同期は以下の差分操作を行う（一方向 Asana → OmniFocus）。
  - 新規（OF に未存在）: OmniFocus タスクを作成する。
  - 更新（双方に存在し差分あり）: name / due / note を更新する。
  - 完了（Asana 側が完了 or 担当解除）: OmniFocus タスクを完了状態にする。
- FR-6: フィールドマッピング
  - Asana `name` → OmniFocus タスク名
  - Asana `due_at`（無ければ `due_on`）→ OmniFocus due date（無ければ未設定）
  - Asana `notes` + `permalink_url` + `asana_gid:` マーカー → OmniFocus note
- FR-7: `--dry-run` 指定時は OmniFocus を変更せず、予定操作を表示する。
- FR-8: 実行後に作成/更新/完了の件数サマリを表示する。

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
