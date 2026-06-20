mod asana;
mod config;
mod model;
mod omnifocus;
mod sync;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;

use crate::asana::AsanaClient;
use crate::config::Config;
use crate::model::Operation;

struct Args {
    dry_run: bool,
    config_path: Option<PathBuf>,
    insecure: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut dry_run = false;
    let mut config_path = None;
    let mut insecure = false;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--insecure" => insecure = true,
            "--config" => {
                let p = it
                    .next()
                    .ok_or_else(|| "--config にはパスが必要です".to_string())?;
                config_path = Some(PathBuf::from(p));
            }
            "-h" | "--help" => {
                println!("usage: asana-omnifocus-sync [--dry-run] [--config <path>] [--insecure]");
                std::process::exit(0);
            }
            other => return Err(format!("不明な引数: {other}")),
        }
    }
    Ok(Args {
        dry_run,
        config_path,
        insecure,
    })
}

fn run() -> Result<()> {
    let args = parse_args().map_err(|e| anyhow::anyhow!(e))?;
    let cfg = Config::load(args.config_path.as_deref())?;

    // 1. Asana から自分担当・未完了タスクを取得
    let insecure = args.insecure || cfg.tls_insecure;
    let client = AsanaClient::new(&cfg.token, &cfg.workspace_gid, insecure)?;
    let asana_tasks = client.my_incomplete_tasks()?;

    // 2. OmniFocus の現状取得
    let of_tasks = omnifocus::dump(&cfg.omnifocus_project)?;

    // 3. 差分計算
    let ops = sync::reconcile(&asana_tasks, &of_tasks);

    let (mut creates, mut updates, mut completes) = (0u32, 0u32, 0u32);
    for op in &ops {
        match op {
            Operation::Create { .. } => creates += 1,
            Operation::Update { .. } => updates += 1,
            Operation::Complete { .. } => completes += 1,
        }
    }

    println!(
        "Asana: {} 件 / OmniFocus: {} 件 / 予定操作: create={creates} update={updates} complete={completes}",
        asana_tasks.len(),
        of_tasks.len(),
    );

    if args.dry_run {
        for op in &ops {
            match op {
                Operation::Create { name, .. } => println!("  [create] {name}"),
                Operation::Update { name, .. } => println!("  [update] {name}"),
                Operation::Complete { of_id } => println!("  [complete] of_id={of_id}"),
            }
        }
        println!("(dry-run: OmniFocus は変更していません)");
        return Ok(());
    }

    if ops.is_empty() {
        println!("変更はありません。");
        return Ok(());
    }

    // 4. OmniFocus に適用
    let summary = omnifocus::apply(&cfg.omnifocus_project, &ops)?;
    println!(
        "完了: created={} updated={} completed={}",
        summary.created, summary.updated, summary.completed
    );

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("エラー: {e:#}");
            ExitCode::FAILURE
        }
    }
}
