mod app;
mod cli;
mod config;
mod engine;
mod llm;
mod tui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands, ConfigAction};
use std::time::Duration;

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
    println!("🛸 DuckPilot - 正在初始化项目空间...\n");
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

/// 启动交互式 TUI 聊天
async fn cmd_chat(project_dir: &std::path::Path) -> Result<()> {
    let mut events = tui::event::EventHandler::new(Duration::from_millis(250));
    let mut app = app::App::new(project_dir.to_path_buf(), events.sender.clone());
    
    // 启动后台扫描
    app.start_scanning();
    
    app.run(&mut events).await?;
    Ok(())
}

/// 数据清洗模式
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

    // 简单的标准输入读取
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

    println!("显示推理过程 y/n (当前: {}):", if settings.show_reasoning { "y" } else { "n" });
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();
    if input == "n" || input == "no" {
        settings.show_reasoning = false;
    } else if !input.is_empty() {
        settings.show_reasoning = true;
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
    println!("  显示推理: {}", if settings.show_reasoning { "是" } else { "否" });
    println!("\n  配置文件: {:?}", config::GlobalSettings::config_path()?);
    Ok(())
}
