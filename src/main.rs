mod cli;
mod config;
mod engine;
mod llm;
mod models;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, ConfigAction};
use colored::*;
use comfy_table::Table;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{self, Write};
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化 color-eyre
    color_eyre::install().expect("无法初始化 color-eyre");

    let cli = Cli::parse();

    // 处理项目目录：如果目录不存在且不是 init 命令，则报错
    let project_dir = if cli.project_dir.exists() {
        std::fs::canonicalize(&cli.project_dir)?
    } else {
        if !matches!(cli.command, Commands::Init) {
            anyhow::bail!("项目目录不存在: {:?}。请先运行 init 命令初始化。", cli.project_dir);
        }
        cli.project_dir.clone()
    };

    match cli.command {
        Commands::Init => cmd_init(&project_dir).await?,
        Commands::Chat => cmd_chat(&project_dir).await?,
        Commands::Clean => cmd_clean(&project_dir).await?,
        Commands::Config { action } => match action {
            ConfigAction::Setup => cmd_config_setup().await?,
            ConfigAction::Show => cmd_config_show()?,
        },
    }

    Ok(())
}

/// 初始化项目空间
async fn cmd_init(project_dir: &std::path::Path) -> Result<()> {
    println!("{}", "🛸 DuckPilot - 正在初始化项目空间...\n".cyan().bold());
    println!("📁 项目目录: {}", project_dir.display());

    // 创建 .duckpilot 目录结构
    config::ProjectConfig::init_project(project_dir)?;
    println!("✅ 项目目录结构已创建");

    // 扫描数据文件
    let data_dir = project_dir.join("data");
    if data_dir.exists() {
        let mut count = 0;
        for entry in std::fs::read_dir(&data_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                match ext.to_lowercase().as_str() {
                    "xlsx" | "xls" | "csv" | "parquet" => {
                        println!("  📄 发现: {}", path.file_name().unwrap_or_default().to_str().unwrap_or(""));
                        count += 1;
                    }
                    _ => {}
                }
            }
        }
        println!("\n📊 共发现 {} 个数据文件", count);
    } else {
        println!("\n⚠️  未找到 data/ 目录，请将数据文件放入 {}", data_dir.display());
        std::fs::create_dir_all(&data_dir)?;
        println!("📁 已创建 data/ 目录");
    }

    // 检查全局配置
    let settings = config::GlobalSettings::load()?;
    if !settings.is_configured() {
        println!("\n⚠️  尚未配置 API Key，请运行: duckpilot config setup");
    }

    println!("\n🎉 初始化完成！运行 `duckpilot chat` 开始分析。");
    Ok(())
}

/// 启动交互式 REPL 聊天
async fn cmd_chat(project_dir: &std::path::Path) -> Result<()> {
    println!("{}", "\n🛸 DuckPilot - 智能数据分析对话".cyan().bold());
    println!("📁 项目: {}", project_dir.display());
    println!("输入 {} 退出，输入 {} 获取帮助\n", "/exit".yellow(), "/help".yellow());

    // 初始化引擎
    let engine = engine::DbEngine::new(project_dir)?;
    let settings = config::GlobalSettings::load()?;
    let llm = llm::LlmClient::new(&settings);

    // 初始扫描数据
    println!("{}", "🔄 正在扫描数据文件...".dimmed());
    let data_dir = project_dir.join("data");
    let schemas = engine.scan_and_register_files(&data_dir)?;
    println!("✅ 已注册 {} 个表结构\n", schemas.len());

    let mut rl = DefaultEditor::new()?;
    let history_path = project_dir.join(".duckpilot").join("history.txt");
    if history_path.exists() {
        let _ = rl.load_history(&history_path);
    }

    loop {
        let readline = rl.readline(&"duckpilot > ".green().bold().to_string());
        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() { continue; }
                
                let _ = rl.add_history_entry(trimmed);

                // 处理命令
                if trimmed.starts_with('/') {
                    match trimmed {
                        "/exit" | "/quit" | "/q" => break,
                        "/help" => {
                            println!("\n可用命令:");
                            println!("  /help    - 显示此帮助");
                            println!("  /clear   - 清屏");
                            println!("  /refresh - 重新扫描数据文件");
                            println!("  /exit    - 退出\n");
                            continue;
                        }
                        "/clear" => {
                            print!("\x1B[2J\x1B[1;1H");
                            let _ = io::stdout().flush();
                            continue;
                        }
                        "/refresh" => {
                            println!("{}", "🔄 正在重新扫描数据文件...".dimmed());
                            let _ = engine.scan_and_register_files(&data_dir);
                            println!("✅ 刷新完成\n");
                            continue;
                        }
                        _ => {
                            println!("未知命令: {}", trimmed.red());
                            continue;
                        }
                    }
                }

                // 处理 Agent 逻辑
                if let Err(e) = handle_agent_query(trimmed, &llm, &engine, &schemas).await {
                    println!("\n{} {}", "❌ 错误:".red().bold(), e);
                }
                println!();
            }
            Err(ReadlineError::Interrupted) => {
                println!("Interrupted");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("EOF");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    let _ = rl.save_history(&history_path);
    Ok(())
}

/// 处理智能体查询逻辑
async fn handle_agent_query(
    question: &str,
    llm: &llm::LlmClient,
    engine: &engine::DbEngine,
    schemas: &[models::TableSchema],
) -> Result<()> {
    println!("\n{}", "💭 思考过程:".dimmed());
    
    // 1. 请求 LLM
    let (content, _) = llm.ask_sql_stream(
        question,
        schemas,
        |chunk| {
            print!("{}", chunk);
            let _ = io::stdout().flush();
        },
        |reasoning| {
            print!("{}", reasoning.dimmed());
            let _ = io::stdout().flush();
        }
    ).await?;
    
    println!("\n");

    // 2. 提取并执行 SQL
    let sql = llm::LlmClient::extract_sql(&content);
    if !sql.is_empty() && !sql.starts_with("--") {
        println!("{}", "📊 执行 SQL:".blue().bold());
        println!("{}\n", sql.cyan());

        match engine.execute_query(&sql) {
            Ok(data) => {
                if data.rows.is_empty() {
                    println!("{}", "查询结果为空".yellow());
                } else {
                    let mut table = Table::new();
                    table.set_header(data.columns);
                    for row in data.rows {
                        table.add_row(row);
                    }
                    println!("{}", table);
                    println!("{}", format!("\n共 {} 行，耗时 {}ms", data.row_count, data.execution_time_ms).dimmed());
                }
            }
            Err(e) => {
                println!("{} {}", "❌ SQL 执行失败:".red().bold(), e);
            }
        }
    }

    Ok(())
}

/// 数据清洗模式 (暂存)
async fn cmd_clean(project_dir: &std::path::Path) -> Result<()> {
    println!("🛸 DuckPilot - 数据清洗模式");
    println!("📁 项目: {}", project_dir.display());
    println!("\n⏳ 数据清洗功能正在开发中...");
    Ok(())
}

/// 配置 API Key 等全局设置
async fn cmd_config_setup() -> Result<()> {
    println!("🛸 DuckPilot - 全局配置\n");

    let mut settings = config::GlobalSettings::load()?;

    println!("请输入 OpenAI API Key (当前: {}):", 
        if settings.api_key.is_empty() { "未设置" } else { "已设置" });
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();
    if !input.is_empty() {
        settings.api_key = input.to_string();
    }

    println!("API Base URL (当前: {}):", settings.api_base);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();
    if !input.is_empty() {
        settings.api_base = input.to_string();
    }

    println!("默认模型 (当前: {}):", settings.model);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim();
    if !input.is_empty() {
        settings.model = input.to_string();
    }

    settings.save()?;
    println!("\n✅ 配置已保存到 {:?}", config::GlobalSettings::config_path()?);
    Ok(())
}

/// 显示当前配置
fn cmd_config_show() -> Result<()> {
    let settings = config::GlobalSettings::load()?;
    println!("🛸 DuckPilot - 当前配置\n");
    println!("  API Key:  {}", if settings.api_key.is_empty() { "未设置" } else { "已设置 ✓" });
    println!("  API Base: {}", settings.api_base);
    println!("  模型:     {}", settings.model);
    println!("  线程数:   {}", settings.max_threads);
    println!("  温度:     {}", settings.temperature);
    println!("\n  配置文件: {:?}", config::GlobalSettings::config_path()?);
    Ok(())
}
