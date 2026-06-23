//! 設定ファイルと環境変数の読み込み。

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

const DEFAULT_PROJECT: &str = "Asana";
const DEFAULT_TAG_ROOT: &str = "Asana";

#[derive(Debug, Deserialize)]
struct ConfigFile {
    workspace_gid: String,
    #[serde(default)]
    omnifocus_project: Option<String>,
    #[serde(default)]
    omnifocus_tag_root: Option<String>,
    #[serde(default)]
    tls_insecure: bool,
}

/// 実行時に確定した設定。
#[derive(Debug, Clone)]
pub struct Config {
    pub token: String,
    pub workspace_gid: String,
    pub omnifocus_project: String,
    pub omnifocus_tag_root: String,
    pub tls_insecure: bool,
}

impl Config {
    /// 設定ファイル（省略時は既定パス）と環境変数 `ASANA_TOKEN` から構築する。
    pub fn load(config_path: Option<&Path>) -> Result<Self> {
        let path = match config_path {
            Some(p) => p.to_path_buf(),
            None => default_config_path()?,
        };

        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("設定ファイルを読めません: {}", path.display()))?;
        let file: ConfigFile = toml::from_str(&raw)
            .with_context(|| format!("設定ファイルの解析に失敗: {}", path.display()))?;

        let token = std::env::var("ASANA_TOKEN")
            .map_err(|_| anyhow!("環境変数 ASANA_TOKEN が未設定です"))?;
        if token.trim().is_empty() {
            return Err(anyhow!("環境変数 ASANA_TOKEN が空です"));
        }

        Ok(Config {
            token,
            workspace_gid: file.workspace_gid,
            omnifocus_project: file
                .omnifocus_project
                .unwrap_or_else(|| DEFAULT_PROJECT.to_string()),
            omnifocus_tag_root: file
                .omnifocus_tag_root
                .unwrap_or_else(|| DEFAULT_TAG_ROOT.to_string()),
            tls_insecure: file.tls_insecure,
        })
    }
}

/// `$XDG_CONFIG_HOME/asana-omnifocus-sync/config.toml`、無ければ `~/.config/...`。
fn default_config_path() -> Result<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| anyhow!("HOME が未設定で設定パスを決定できません"))?;
            PathBuf::from(home).join(".config")
        }
    };
    Ok(base.join("asana-omnifocus-sync").join("config.toml"))
}
