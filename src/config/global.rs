use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 全局配置，存储在 ~/.duckpilot/settings.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// OpenAI API 密钥
    pub api_key: String,

    /// API 基础 URL（支持兼容 API，如 Ollama / DeepSeek）
    #[serde(default = "default_api_base")]
    pub api_base: String,

    /// 默认使用的模型名称
    #[serde(default = "default_model")]
    pub model: String,

    /// 最大并发线程数
    #[serde(default = "default_max_threads")]
    pub max_threads: usize,

    /// 温度参数（0.0-2.0）
    #[serde(default = "default_temperature")]
    pub temperature: f32,
}

fn default_api_base() -> String {
    "https://api.openai.com/v1".to_string()
}

fn default_model() -> String {
    "gpt-4o".to_string()
}

fn default_max_threads() -> usize {
    4
}

fn default_temperature() -> f32 {
    0.0
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: default_api_base(),
            model: default_model(),
            max_threads: default_max_threads(),
            temperature: default_temperature(),
        }
    }
}

impl GlobalSettings {
    /// 获取全局配置目录路径
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("无法获取用户主目录")?;
        Ok(home.join(".duckpilot"))
    }

    /// 获取配置文件路径
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("settings.json"))
    }

    /// 从文件加载全局配置
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("无法读取配置文件: {}", path.display()))?;
        let settings: Self = serde_json::from_str(&content)
            .with_context(|| "配置文件格式错误")?;
        Ok(settings)
    }

    /// 保存全局配置到文件
    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir()?;
        std::fs::create_dir_all(&dir)?;
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)
            .with_context(|| format!("无法写入配置文件: {}", path.display()))?;
        Ok(())
    }

    /// 检查配置是否有效（API Key 已设置）
    pub fn is_configured(&self) -> bool {
        !self.api_key.is_empty()
    }
}
