use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// 🛸 DuckPilot - 本地数据分析 Agent
/// 
/// 基于 DuckDB 计算引擎、LLM 驱动的智能数据分析工具
#[derive(Parser, Debug)]
#[command(name = "duckpilot")]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// 项目目录路径（默认为当前目录）
    #[arg(short = 'p', long, default_value = ".")]
    pub project_dir: PathBuf,

    /// 启用详细日志输出
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 初始化项目空间：扫描数据文件、生成 Schema、创建配置
    Init,

    /// 启动交互式 TUI 分析界面
    Chat,

    /// 数据清洗模式：检测并修复数据质量问题
    Clean,

    /// 管理全局设置（API Key、模型配置等）
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// 交互式设置全局配置
    Setup,
    /// 显示当前配置
    Show,
}
