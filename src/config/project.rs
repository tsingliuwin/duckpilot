use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 项目配置，存储在 <project_root>/config.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// 项目名称
    #[serde(default)]
    pub name: String,

    /// 行业/业务背景描述
    #[serde(default)]
    pub industry: String,

    /// 分析师角色描述
    #[serde(default)]
    pub analyst_role: String,

    /// 自定义业务指标定义
    #[serde(default)]
    pub metrics: Vec<MetricDefinition>,

    /// 数据清洗规则
    #[serde(default)]
    pub cleaning_rules: Vec<CleaningRule>,

    /// 自定义视图定义
    #[serde(default)]
    pub views: Vec<ViewDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDefinition {
    /// 指标名称（如 "复购率"）
    pub name: String,
    /// 指标计算公式的自然语言描述
    pub description: String,
    /// 对应的 SQL 表达式（可选）
    #[serde(default)]
    pub sql_expression: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleaningRule {
    /// 规则名称
    pub name: String,
    /// 目标表/文件
    pub target: String,
    /// 规则描述
    pub description: String,
    /// 对应的 SQL 转换
    #[serde(default)]
    pub sql_transform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDefinition {
    /// 视图名称
    pub name: String,
    /// 基于的源表
    pub source: String,
    /// 视图 SQL
    pub sql: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            industry: String::new(),
            analyst_role: "数据分析师".to_string(),
            metrics: Vec::new(),
            cleaning_rules: Vec::new(),
            views: Vec::new(),
        }
    }
}

impl ProjectConfig {
    /// DuckPilot 隐藏目录名
    const DUCKPILOT_DIR: &'static str = ".duckpilot";
    const CONFIG_FILE: &'static str = "config.yaml";

    /// 获取项目的 .duckpilot 目录
    pub fn duckpilot_dir(project_root: &Path) -> PathBuf {
        project_root.join(Self::DUCKPILOT_DIR)
    }

    /// 获取配置文件路径
    pub fn config_path(project_root: &Path) -> PathBuf {
        project_root.join(Self::CONFIG_FILE)
    }

    /// 检查指定目录是否已初始化为 DuckPilot 项目
    pub fn is_initialized(project_root: &Path) -> bool {
        Self::duckpilot_dir(project_root).exists()
    }

    /// 初始化项目目录结构
    pub fn init_project(project_root: &Path) -> Result<()> {
        let dp_dir = Self::duckpilot_dir(project_root);
        std::fs::create_dir_all(&dp_dir)?;
        std::fs::create_dir_all(dp_dir.join("pilot_log"))?;

        // 如果 config.yaml 不存在，创建默认配置
        let config_path = Self::config_path(project_root);
        if !config_path.exists() {
            let default_config = Self::default();
            default_config.save(project_root)?;
        }
        Ok(())
    }

    /// 从文件加载项目配置
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = Self::config_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("无法读取项目配置: {}", path.display()))?;
        let config: Self = serde_yaml::from_str(&content)
            .with_context(|| "项目配置格式错误")?;
        Ok(config)
    }

    /// 保存项目配置到文件
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let path = Self::config_path(project_root);
        let content = serde_yaml::to_string(self)?;
        std::fs::write(&path, content)
            .with_context(|| format!("无法写入项目配置: {}", path.display()))?;
        Ok(())
    }
}
