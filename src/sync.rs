//! Asana と OmniFocus の突き合わせ・差分計算（純粋関数）。

use std::collections::{HashMap, HashSet};

use crate::model::{AsanaOp, AsanaTask, OfTask, Operation, Plan};

/// Asana タスク集合と OmniFocus タスク集合から、適用計画を計算する。
///
/// OmniFocus 行きの操作（`of_ops`）:
/// - Asana にあり OF（未完了にも完了済みにも）無い → Create
/// - 双方（OF 未完了）に存在し name/due/note/tags に差分あり → Update
/// - OF（未完了）にあり Asana に無い → Complete
///
/// Asana 行きの操作（`asana_ops`、完了の書き戻し）:
/// - OF（完了済み）にあり Asana には未完了で残っている → Complete（OF への Create は出さない）
///
/// OF（完了済み）かつ Asana にも無い（双方完了済み）は何もしない（冪等）。
pub fn reconcile(asana: &[AsanaTask], of: &[OfTask]) -> Plan {
    let of_by_gid: HashMap<&str, &OfTask> = of
        .iter()
        .filter(|t| !t.completed)
        .map(|t| (t.asana_gid.as_str(), t))
        .collect();

    let completed_gids: HashSet<&str> = of
        .iter()
        .filter(|t| t.completed)
        .map(|t| t.asana_gid.as_str())
        .collect();

    let asana_gids: HashMap<&str, &AsanaTask> =
        asana.iter().map(|t| (t.gid.as_str(), t)).collect();

    let mut plan = Plan::default();

    // Create / Update / 完了の書き戻し
    for a in asana {
        let note = a.note();
        match of_by_gid.get(a.gid.as_str()) {
            None => {
                if completed_gids.contains(a.gid.as_str()) {
                    // OF で完了済みだが Asana は未完了 → Asana へ書き戻し、再作成はしない。
                    plan.asana_ops.push(AsanaOp::Complete {
                        gid: a.gid.clone(),
                        name: a.name.clone(),
                    });
                } else {
                    plan.of_ops.push(Operation::Create {
                        asana_gid: a.gid.clone(),
                        name: a.name.clone(),
                        due: a.due.clone(),
                        note,
                        tags: a.projects.clone(),
                    });
                }
            }
            Some(o) => {
                if o.name != a.name
                    || o.due != a.due
                    || o.note != note
                    || tags_differ(&a.projects, &o.tags)
                {
                    plan.of_ops.push(Operation::Update {
                        of_id: o.of_id.clone(),
                        name: a.name.clone(),
                        due: a.due.clone(),
                        note,
                        tags: a.projects.clone(),
                    });
                }
            }
        }
    }

    // Complete: OF（未完了）にあるが Asana に無いもの
    for o in of.iter().filter(|t| !t.completed) {
        if !asana_gids.contains_key(o.asana_gid.as_str()) {
            plan.of_ops.push(Operation::Complete {
                of_id: o.of_id.clone(),
                name: o.name.clone(),
            });
        }
    }

    plan
}

/// 所属プロジェクト名と管理対象タグを、順序を無視した集合として比較する。
fn tags_differ(projects: &[String], tags: &[String]) -> bool {
    let want: HashSet<&str> = projects.iter().map(String::as_str).collect();
    let have: HashSet<&str> = tags.iter().map(String::as_str).collect();
    want != have
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asana(gid: &str, name: &str, due: Option<&str>) -> AsanaTask {
        AsanaTask {
            gid: gid.to_string(),
            name: name.to_string(),
            due: due.map(str::to_string),
            notes: String::new(),
            permalink_url: format!("https://app.asana.com/0/0/{gid}"),
            projects: Vec::new(),
        }
    }

    fn of_from(a: &AsanaTask, of_id: &str, completed: bool) -> OfTask {
        OfTask {
            of_id: of_id.to_string(),
            asana_gid: a.gid.clone(),
            name: a.name.clone(),
            due: a.due.clone(),
            completed,
            note: a.note(),
            tags: a.projects.clone(),
        }
    }

    #[test]
    fn creates_when_missing_in_of() {
        let a = asana("1", "新規", Some("2026-06-30"));
        let plan = reconcile(std::slice::from_ref(&a), &[]);
        assert!(plan.asana_ops.is_empty());
        assert!(matches!(plan.of_ops.as_slice(), [Operation::Create { .. }]));
    }

    #[test]
    fn updates_when_name_differs() {
        let a = asana("1", "新しい名前", None);
        let mut of = of_from(&a, "of-1", false);
        of.name = "古い名前".to_string();
        let plan = reconcile(&[a], &[of]);
        assert!(matches!(plan.of_ops.as_slice(), [Operation::Update { .. }]));
    }

    #[test]
    fn no_op_when_identical() {
        let a = asana("1", "同じ", Some("2026-06-30"));
        let of = of_from(&a, "of-1", false);
        assert_eq!(reconcile(&[a], &[of]), Plan::default());
    }

    #[test]
    fn completes_when_missing_in_asana() {
        let a = asana("1", "消えた", None);
        let of = of_from(&a, "of-1", false);
        let plan = reconcile(&[], &[of]);
        assert!(plan.asana_ops.is_empty());
        assert!(matches!(
            plan.of_ops.as_slice(),
            [Operation::Complete { of_id, name }] if of_id == "of-1" && name == "消えた"
        ));
    }

    #[test]
    fn ignores_already_completed_of_task() {
        let a = asana("1", "完了済み", None);
        let of = of_from(&a, "of-1", true);
        // Asana にも無く、OF は完了済み → 双方完了済み。何もしない（再オープンも書き戻しもしない）。
        assert_eq!(reconcile(&[], &[of]), Plan::default());
    }

    #[test]
    fn writes_back_complete_when_of_completed_but_asana_incomplete() {
        // OF で完了済みかつ Asana には未完了で残る → Asana へ完了を書き戻し、OF への再作成はしない。
        let a = asana("1", "OFで完了", None);
        let of = of_from(&a, "of-1", true);
        let plan = reconcile(std::slice::from_ref(&a), &[of]);
        assert!(plan.of_ops.is_empty());
        assert!(matches!(
            plan.asana_ops.as_slice(),
            [AsanaOp::Complete { gid, .. }] if gid == "1"
        ));
    }

    #[test]
    fn create_carries_project_tags() {
        let mut a = asana("1", "新規", None);
        a.projects = vec!["プロジェクト A".to_string(), "プロジェクト B".to_string()];
        let plan = reconcile(std::slice::from_ref(&a), &[]);
        assert!(matches!(
            plan.of_ops.as_slice(),
            [Operation::Create { tags, .. }] if tags == &a.projects
        ));
    }

    #[test]
    fn updates_when_tags_differ() {
        let mut a = asana("1", "同じ", None);
        a.projects = vec!["プロジェクト A".to_string()];
        let mut of = of_from(&a, "of-1", false);
        of.tags = vec!["プロジェクト B".to_string()];
        let plan = reconcile(&[a], &[of]);
        assert!(matches!(
            plan.of_ops.as_slice(),
            [Operation::Update { tags, .. }] if tags == &["プロジェクト A".to_string()]
        ));
    }

    #[test]
    fn no_op_when_tags_match_ignoring_order() {
        let mut a = asana("1", "同じ", None);
        a.projects = vec!["A".to_string(), "B".to_string()];
        let mut of = of_from(&a, "of-1", false);
        of.tags = vec!["B".to_string(), "A".to_string()];
        assert_eq!(reconcile(&[a], &[of]), Plan::default());
    }

    #[test]
    fn updates_when_project_membership_removed() {
        // Asana で全プロジェクトから外れたら、管理対象タグを空へ更新する。
        let a = asana("1", "解除", None);
        let mut of = of_from(&a, "of-1", false);
        of.tags = vec!["プロジェクト A".to_string()];
        let plan = reconcile(&[a], &[of]);
        assert!(matches!(
            plan.of_ops.as_slice(),
            [Operation::Update { tags, .. }] if tags.is_empty()
        ));
    }
}
