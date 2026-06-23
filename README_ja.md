# asana-omnifocus-sync

[English](README.md) | 日本語

Asana で自分に割り当てられた未完了タスクを、OmniFocus の指定プロジェクトへ一方向同期する CLI です。

OmniFocus のタスクに同期元の Asana タスク GID を埋め込み、それを正本として突き合わせます。
ツール自身はデータベースや状態ファイルを持たないため、何度実行しても結果は同じになります。

macOS 専用です。
OmniFocus との連携に JXA（JavaScript for Automation）を使います。

## 同期の挙動

実行のたびに、Asana の現状を OmniFocus へ反映します。

- Asana にあり OmniFocus に無いタスクを作成する
- 双方にあり、名前・期日・メモ・プロジェクトタグのいずれかが異なるタスクを更新する
- OmniFocus（未完了）にあるが Asana の取得結果に無いタスクを完了にする

Asana からは「現在自分に割り当てられた未完了タスク」だけを取得します。
完了したタスクや担当を外れたタスクは取得結果に現れません。
そのため OmniFocus 側に残っているそれらのタスクを、ツールが完了として扱います。

OmniFocus 側で先に完了させたタスクは突き合わせの対象から外し、再び開くことはありません。

## Asana プロジェクトを OmniFocus のタグにする

Asana のタスクは複数のプロジェクトに同時所属できますが、OmniFocus のタスクは 1 つのプロジェクトにしか属せません。
この差を埋めるため、Asana プロジェクトを OmniFocus のプロジェクトではなくタグで表現します。
タスクの配置は 1 つの取り込み先プロジェクトのままにし、所属プロジェクトはタグで表します。

タグはルートタグ（既定 `Asana`）の配下にプロジェクト名でネストします。
複数の Asana プロジェクトに所属するタスクには、対応するタグを複数付与します。
どのプロジェクトにも属さないタスクには、ルートタグのみを付与します。

同期が管理するのはルートタグ配下のタグだけです。
Asana 側で所属プロジェクトが変わると、当該タスクの管理対象タグはそれに合わせて置き換わります。
利用者が手動で付けた他のタグ（コンテキストや場所など）は保持します。
使われなくなったタグは削除せず残し、Asana のプロジェクト名のリネームは別タグとして扱います。

## 必要なもの

- macOS と OmniFocus
- Asana の個人アクセストークン（[Asana の開発者コンソール](https://app.asana.com/0/my-apps)で発行）
- Rust ツールチェイン、または nix

## インストール

nix を使う場合は次のコマンドでビルドできます。
成果物は単一バイナリで、`./result/bin/asana-omnifocus-sync` に置かれます。

```
nix build
```

Rust ツールチェインを直接使う場合は次のとおりです。

```
cargo build --release
```

## 設定

設定ファイルを `~/.config/asana-omnifocus-sync/config.toml` に置きます（`XDG_CONFIG_HOME` を尊重します）。
`config.example.toml` を雛形として利用できます。

```toml
workspace_gid = "1234567890"   # 対象 Asana ワークスペースの GID（必須）
omnifocus_project = "Asana"    # 取り込み先 OmniFocus プロジェクト名（省略時 "Asana"）
omnifocus_tag_root = "Asana"   # Asana プロジェクトを表すタグのルートタグ名（省略時 "Asana"）
tls_insecure = false           # true で TLS 証明書検証を無効化（省略時 false）
```

認証トークンは設定ファイルに書かず、環境変数 `ASANA_TOKEN` で渡します。

```
export ASANA_TOKEN="<your-personal-access-token>"
```

ワークスペースの GID は、Asana を開いたときの URL（`https://app.asana.com/0/<gid>/...`）から確認できます。

## 使い方

まず `--dry-run` で予定される操作を確認します。
このモードでは OmniFocus を変更しません。

```
asana-omnifocus-sync --dry-run
```

問題なければ、そのまま実行して適用します。

```
asana-omnifocus-sync
```

終了時に `created=N updated=N completed=N` の形式でサマリを表示します。

### オプション

- `--dry-run`：適用せず、予定操作だけを表示する
- `--config <path>`：設定ファイルのパスを上書きする
- `--insecure`：TLS 証明書検証を無効化する（設定の `tls_insecure` より優先）

## TLS と社内プロキシ

HTTPS 通信は native-tls（macOS では Security.framework）を使い、システムキーチェーンのルート証明書を信頼します。
Netskope などの TLS インターセプトプロキシが挿入する社内 CA も、キーチェーンに導入済みであれば証明書を検証したまま受け入れます。

キーチェーンに社内 CA が無いなどで検証できないときは、`tls_insecure = true` または `--insecure` で検証を無効化できます。
検証を切るため、信頼できるネットワークでのみ使ってください。

## 開発

開発は nix flake の devShell 上で行います（`.envrc` で direnv が `use flake` を読み込みます）。

```
cargo test       # 突き合わせロジック（reconcile）のユニットテスト
cargo clippy --all-targets -- --deny warnings
nix flake check  # clippy・テスト・ビルドを一括検証
```

設計と詳細仕様は `docs/` にあります。

- `docs/use-cases.md`：ユースケース
- `docs/requirements.md`：要件
- `docs/design.md`：設計（モジュール構成とデータモデル）
- `docs/spec.md`：詳細仕様（API パラメータ、note フォーマット、同期ルール）
