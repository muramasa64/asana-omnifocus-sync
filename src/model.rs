//! Asana / OmniFocus 双方を表す共通モデルと、マッピング用の純粋関数。

use serde::{Deserialize, Serialize};

/// Asana から取得し、正規化済みの自分担当・未完了タスク。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsanaTask {
    pub gid: String,
    pub name: String,
    /// 正規化済み due（"YYYY-MM-DD" もしくは RFC3339）。未設定なら None。
    pub due: Option<String>,
    pub notes: String,
    pub permalink_url: String,
    /// 所属プロジェクト名（OmniFocus のタグに対応）。どこにも属さなければ空。
    pub projects: Vec<String>,
}

/// OmniFocus 側の現状タスク（dump.js の出力に対応）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OfTask {
    pub of_id: String,
    pub asana_gid: String,
    pub name: String,
    pub due: Option<String>,
    pub completed: bool,
    pub note: String,
    /// 管理対象タグ（ルートタグ配下の子タグ名）。
    #[serde(default)]
    pub tags: Vec<String>,
}

/// reconcile が生成する、OmniFocus へ適用する操作。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Operation {
    Create {
        asana_gid: String,
        name: String,
        due: Option<String>,
        note: String,
        tags: Vec<String>,
    },
    Update {
        of_id: String,
        name: String,
        due: Option<String>,
        note: String,
        tags: Vec<String>,
    },
    Complete {
        of_id: String,
    },
}

/// reconcile が生成する、Asana へ適用する操作（完了の書き戻し）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsanaOp {
    /// 当該 gid の Asana タスクを完了にする。`name` は表示用。
    Complete { gid: String, name: String },
}

/// reconcile の出力。適用先ごとに操作を分ける。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// OmniFocus へ適用する操作。
    pub of_ops: Vec<Operation>,
    /// Asana へ適用する操作（完了の書き戻し）。
    pub asana_ops: Vec<AsanaOp>,
}

/// Asana の due_at（日時）/ due_on（日付）から OmniFocus 用の due を決める。
///
/// OmniFocus の due は日時だが、Asana の主用途は日付（due_on）であり、
/// 日時で突き合わせると再実行のたびに差分扱いになる。MVP では due を
/// 日付粒度（"YYYY-MM-DD"）に正規化して比較を安定させる。due_at の時刻は捨てる。
pub fn normalize_due(due_on: Option<&str>, due_at: Option<&str>) -> Option<String> {
    let pick = match (due_at, due_on) {
        (Some(at), _) if !at.is_empty() => at,
        (_, Some(on)) if !on.is_empty() => on,
        _ => return None,
    };
    // 先頭の "YYYY-MM-DD" 部分のみを採る。
    Some(pick.chars().take(10).collect())
}

/// OmniFocus タスクの note を構築する。`asana_gid:` 行が対応付けの正本。
pub fn build_note(notes: &str, permalink_url: &str, gid: &str) -> String {
    let body = notes.trim_end();
    format!(
        "{body}{sep}---\nasana_url: {permalink_url}\nasana_gid: {gid}\n",
        sep = if body.is_empty() { "" } else { "\n\n" },
    )
}

impl AsanaTask {
    /// この Asana タスクに対応する OmniFocus note 文字列。
    pub fn note(&self) -> String {
        build_note(&self.notes, &self.permalink_url, &self.gid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_due_prefers_due_at() {
        assert_eq!(
            normalize_due(Some("2026-06-30"), Some("2026-06-30T09:00:00Z")),
            Some("2026-06-30".to_string())
        );
        assert_eq!(
            normalize_due(Some("2026-06-30"), None),
            Some("2026-06-30".to_string())
        );
        assert_eq!(normalize_due(None, None), None);
        assert_eq!(normalize_due(Some(""), Some("")), None);
    }

    #[test]
    fn build_note_embeds_marker() {
        let note = build_note("memo", "https://example/123", "123");
        assert!(note.contains("memo"));
        assert!(note.contains("asana_url: https://example/123"));
        assert!(note.contains("asana_gid: 123"));
    }

    #[test]
    fn build_note_handles_empty_body() {
        let note = build_note("", "https://example/9", "9");
        assert!(note.starts_with("---\n"));
        assert!(note.contains("asana_gid: 9"));
    }
}
