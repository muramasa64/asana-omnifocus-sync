mod asana;
mod config;
mod model;
mod omnifocus;
mod sync;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};

use crate::asana::AsanaClient;
use crate::config::Config;
use crate::model::{AsanaOp, Operation};

struct Args {
    dry_run: bool,
    verbose: bool,
    config_path: Option<PathBuf>,
    insecure: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut dry_run = false;
    let mut verbose = false;
    let mut config_path = None;
    let mut insecure = false;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--verbose" | "-v" => verbose = true,
            "--insecure" => insecure = true,
            "--config" => {
                let p = it
                    .next()
                    .ok_or_else(|| "--config にはパスが必要です".to_string())?;
                config_path = Some(PathBuf::from(p));
            }
            "-h" | "--help" => {
                println!("usage: asana-omnifocus-sync [--dry-run] [--verbose] [--config <path>] [--insecure]");
                std::process::exit(0);
            }
            other => return Err(format!("不明な引数: {other}")),
        }
    }
    Ok(Args {
        dry_run,
        verbose,
        config_path,
        insecure,
    })
}

/// 予定操作の件名を一覧表示する（`--dry-run` および `--verbose` 時）。
fn print_operations(of_ops: &[Operation], asana_ops: &[AsanaOp]) {
    for op in of_ops {
        match op {
            Operation::Create { name, .. } => println!("  [create] {name}"),
            Operation::Update { name, .. } => println!("  [update] {name}"),
            Operation::Complete { name, .. } => println!("  [complete] {name}"),
        }
    }
    for AsanaOp::Complete { gid, name } in asana_ops {
        println!("  [asana complete] {name} (gid={gid})");
    }
}

fn run() -> Result<()> {
    let args = parse_args().map_err(|e| anyhow::anyhow!(e))?;
    let cfg = Config::load(args.config_path.as_deref())?;

    // 1. Asana から自分担当・未完了タスクを取得
    let insecure = args.insecure || cfg.tls_insecure;
    let client = AsanaClient::new(&cfg.token, &cfg.workspace_gid, insecure)?;
    let asana_tasks = client.my_incomplete_tasks()?;

    // 2. OmniFocus の現状取得
    let of_tasks = omnifocus::dump(&cfg.omnifocus_project, &cfg.omnifocus_tag_root)?;

    // 3. 差分計算
    let plan = sync::reconcile(&asana_tasks, &of_tasks);

    let (mut creates, mut updates, mut completes) = (0u32, 0u32, 0u32);
    for op in &plan.of_ops {
        match op {
            Operation::Create { .. } => creates += 1,
            Operation::Update { .. } => updates += 1,
            Operation::Complete { .. } => completes += 1,
        }
    }
    let writebacks = plan.asana_ops.len();

    println!(
        "Asana: {} 件 / OmniFocus: {} 件 / 予定操作: create={creates} update={updates} complete={completes} asana_complete={writebacks}",
        asana_tasks.len(),
        of_tasks.len(),
    );

    if args.dry_run {
        print_operations(&plan.of_ops, &plan.asana_ops);
        println!("(dry-run: OmniFocus / Asana ともに変更していません)");
        return Ok(());
    }

    if plan.of_ops.is_empty() && plan.asana_ops.is_empty() {
        println!("変更はありません。");
        return Ok(());
    }

    // 4. OmniFocus に適用
    let summary = if plan.of_ops.is_empty() {
        Default::default()
    } else {
        omnifocus::apply(&cfg.omnifocus_project, &cfg.omnifocus_tag_root, &plan.of_ops)?
    };

    // 5. Asana へ完了を書き戻す
    let mut asana_completed = 0u32;
    for AsanaOp::Complete { gid, name } in &plan.asana_ops {
        client
            .complete_task(gid)
            .with_context(|| format!("Asana タスクの完了に失敗: {name} (gid={gid})"))?;
        asana_completed += 1;
    }

    if args.verbose {
        print_operations(&plan.of_ops, &plan.asana_ops);
    }

    println!(
        "完了: created={} updated={} completed={} asana_completed={asana_completed}",
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
