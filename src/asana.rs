//! Asana REST クライアント（自分担当・未完了タスクの取得）。

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::model::{normalize_due, AsanaTask};

const API_BASE: &str = "https://app.asana.com/api/1.0";
const OPT_FIELDS: &str = "name,due_on,due_at,notes,permalink_url,completed,assignee.gid";

#[derive(Debug, Deserialize)]
struct TasksResponse {
    data: Vec<RawTask>,
    #[serde(default)]
    next_page: Option<NextPage>,
}

#[derive(Debug, Deserialize)]
struct NextPage {
    offset: String,
}

#[derive(Debug, Deserialize)]
struct RawTask {
    gid: String,
    name: Option<String>,
    due_on: Option<String>,
    due_at: Option<String>,
    notes: Option<String>,
    permalink_url: Option<String>,
}

pub struct AsanaClient<'a> {
    token: &'a str,
    workspace_gid: &'a str,
    agent: ureq::Agent,
}

impl<'a> AsanaClient<'a> {
    /// `insecure=true` のとき TLS 証明書検証を無効化する。
    pub fn new(token: &'a str, workspace_gid: &'a str, insecure: bool) -> Result<Self> {
        let mut builder = native_tls::TlsConnector::builder();
        if insecure {
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
        }
        let connector = builder
            .build()
            .context("TLS コネクタの初期化に失敗")?;
        let agent = ureq::AgentBuilder::new()
            .tls_connector(std::sync::Arc::new(connector))
            .build();

        Ok(Self {
            token,
            workspace_gid,
            agent,
        })
    }

    /// 自分に割り当てられた未完了タスクを全件（ページネーション込み）取得する。
    pub fn my_incomplete_tasks(&self) -> Result<Vec<AsanaTask>> {
        let mut tasks = Vec::new();
        let mut offset: Option<String> = None;

        loop {
            let mut req = self
                .agent
                .get(&format!("{API_BASE}/tasks"))
                .set("Authorization", &format!("Bearer {}", self.token))
                .query("assignee", "me")
                .query("workspace", self.workspace_gid)
                .query("completed_since", "now")
                .query("opt_fields", OPT_FIELDS)
                .query("limit", "100");
            if let Some(o) = &offset {
                req = req.query("offset", o);
            }

            let resp = req.call().map_err(map_ureq_err)?;
            let body: TasksResponse = resp
                .into_json()
                .context("Asana レスポンスの JSON 解析に失敗")?;

            for raw in body.data {
                tasks.push(AsanaTask {
                    gid: raw.gid,
                    name: raw.name.unwrap_or_default(),
                    due: normalize_due(raw.due_on.as_deref(), raw.due_at.as_deref()),
                    notes: raw.notes.unwrap_or_default(),
                    permalink_url: raw.permalink_url.unwrap_or_default(),
                });
            }

            match body.next_page {
                Some(np) => offset = Some(np.offset),
                None => break,
            }
        }

        Ok(tasks)
    }
}

/// ureq のエラーを、HTTP ステータス本文を含む anyhow エラーへ変換する。
fn map_ureq_err(err: ureq::Error) -> anyhow::Error {
    match err {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            anyhow!("Asana API エラー (HTTP {code}): {body}")
        }
        ureq::Error::Transport(t) => anyhow!("Asana API への通信に失敗: {t}"),
    }
}
