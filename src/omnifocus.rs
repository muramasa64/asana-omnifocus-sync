//! OmniFocus 連携。埋め込んだ JXA スクリプトを `osascript` 経由で実行する。

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::model::{OfTask, Operation};

const OSASCRIPT: &str = "/usr/bin/osascript";
const DUMP_JS: &str = include_str!("../scripts/dump.js");
const APPLY_JS: &str = include_str!("../scripts/apply.js");

/// apply.js が返す適用結果のサマリ。
#[derive(Debug, Clone, Deserialize)]
pub struct ApplySummary {
    pub created: u32,
    pub updated: u32,
    pub completed: u32,
}

/// 取り込み先プロジェクト配下の、`asana_gid:` を持つ OmniFocus タスクを取得する。
pub fn dump(project: &str) -> Result<Vec<OfTask>> {
    let script = write_temp_script("dump", DUMP_JS)?;
    let output = Command::new(OSASCRIPT)
        .arg("-l")
        .arg("JavaScript")
        .arg(&script)
        .arg(project)
        .output()
        .context("osascript の実行に失敗（dump）")?;

    let _ = std::fs::remove_file(&script);

    if !output.status.success() {
        return Err(anyhow!(
            "dump.js 実行エラー: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout).context("dump 出力が UTF-8 ではありません")?;
    serde_json::from_str(stdout.trim()).with_context(|| format!("dump 出力の解析に失敗: {stdout}"))
}

/// 操作リストを OmniFocus に適用する。
pub fn apply(project: &str, operations: &[Operation]) -> Result<ApplySummary> {
    let payload = serde_json::json!({
        "project": project,
        "operations": operations,
    });
    let payload = serde_json::to_string(&payload)?;

    let script = write_temp_script("apply", APPLY_JS)?;
    let mut child = Command::new(OSASCRIPT)
        .arg("-l")
        .arg("JavaScript")
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("osascript の起動に失敗（apply）")?;

    child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("apply の stdin を取得できません"))?
        .write_all(payload.as_bytes())
        .context("apply への入力書き込みに失敗")?;

    let output = child
        .wait_with_output()
        .context("osascript の終了待ちに失敗（apply）")?;

    let _ = std::fs::remove_file(&script);

    if !output.status.success() {
        return Err(anyhow!(
            "apply.js 実行エラー: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout).context("apply 出力が UTF-8 ではありません")?;
    serde_json::from_str(stdout.trim())
        .with_context(|| format!("apply 出力の解析に失敗: {stdout}"))
}

/// 埋め込みスクリプトを一時ファイルに書き出し、そのパスを返す。
fn write_temp_script(name: &str, contents: &str) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "asana-omnifocus-sync-{name}-{}.js",
        std::process::id()
    ));
    std::fs::write(&path, contents)
        .with_context(|| format!("一時スクリプトの書き出しに失敗: {}", path.display()))?;
    Ok(path)
}
