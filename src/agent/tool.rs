use anyhow::Result;
use std::collections::HashMap;

use crate::engine::DbEngine;

/// 工具名称、描述、参数（JSON Schema）
pub trait ToolInfo: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
}

/// 所有内置工具的枚举
pub enum AgentTool {
    ListTables(tools::ListTablesTool),
    DescribeTable(tools::DescribeTableTool),
    ExecuteQuery(tools::ExecuteQueryTool),
    SampleData(tools::SampleDataTool),
    RepairTable(tools::RepairTableTool),
}

impl ToolInfo for AgentTool {
    fn name(&self) -> &str {
        match self {
            Self::ListTables(t) => t.name(),
            Self::DescribeTable(t) => t.name(),
            Self::ExecuteQuery(t) => t.name(),
            Self::SampleData(t) => t.name(),
            Self::RepairTable(t) => t.name(),
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::ListTables(t) => t.description(),
            Self::DescribeTable(t) => t.description(),
            Self::ExecuteQuery(t) => t.description(),
            Self::SampleData(t) => t.description(),
            Self::RepairTable(t) => t.description(),
        }
    }

    fn parameters(&self) -> serde_json::Value {
        match self {
            Self::ListTables(t) => t.parameters(),
            Self::DescribeTable(t) => t.parameters(),
            Self::ExecuteQuery(t) => t.parameters(),
            Self::SampleData(t) => t.parameters(),
            Self::RepairTable(t) => t.parameters(),
        }
    }
}

impl AgentTool {
    /// 执行工具（同步，因为 DuckDB 操作本身是同步的）
    pub fn execute(&self, db: &DbEngine, args: serde_json::Value) -> Result<String> {
        match self {
            Self::ListTables(t) => t.execute(db, args),
            Self::DescribeTable(t) => t.execute(db, args),
            Self::ExecuteQuery(t) => t.execute(db, args),
            Self::SampleData(t) => t.execute(db, args),
            Self::RepairTable(t) => t.execute(db, args),
        }
    }
}

/// 工具注册表
pub struct ToolRegistry {
    tools: HashMap<String, AgentTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: AgentTool) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&AgentTool> {
        self.tools.get(name)
    }

    /// 生成 OpenAI tools API 格式的工具列表
    pub fn to_api_tools(&self) -> Vec<serde_json::Value> {
        let mut tools: Vec<_> = self.tools.values().collect();
        tools.sort_by_key(|t| t.name());
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": t.parameters(),
                    }
                })
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 构建包含所有内置工具的注册表
pub fn build_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(AgentTool::ListTables(tools::ListTablesTool));
    reg.register(AgentTool::DescribeTable(tools::DescribeTableTool));
    reg.register(AgentTool::ExecuteQuery(tools::ExecuteQueryTool));
    reg.register(AgentTool::SampleData(tools::SampleDataTool));
    reg.register(AgentTool::RepairTable(tools::RepairTableTool));
    reg
}

// 子模块
pub mod tools {
    use anyhow::Result;
    use crate::engine::DbEngine;

    fn required_str(args: &serde_json::Value, key: &str) -> Result<String> {
        args.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("缺少必填参数: {}", key))
    }

    pub struct ListTablesTool;
    impl ListTablesTool {
        pub const NAME: &'static str = "list_tables";
        pub const DESC: &'static str = "列出数据库中所有可用的表名及行数。当你需要了解有哪些数据可用时使用此工具。";
        pub fn name(&self) -> &str { Self::NAME }
        pub fn description(&self) -> &str { Self::DESC }
        pub fn parameters(&self) -> serde_json::Value {
            serde_json::json!({ "type": "object", "properties": {}, "required": [] })
        }
        pub fn execute(&self, db: &DbEngine, _args: serde_json::Value) -> Result<String> {
            let result = db.execute_query(
                "SELECT table_name FROM duckdb_tables() WHERE schema_name = 'main' ORDER BY table_name"
            )?;
            let mut output = String::from("可用表列表：\n");
            for row in &result.rows {
                let table_name = row.first().map(|s| s.as_str()).unwrap_or("?");
                output.push_str(&format!("  - {}\n", table_name));
            }
            if result.rows.is_empty() {
                output.push_str("  （无表）\n");
            }
            Ok(output)
        }
    }

    pub struct DescribeTableTool;
    impl DescribeTableTool {
        pub const NAME: &'static str = "describe_table";
        pub const DESC: &'static str = "查看指定表的结构信息，包括列名、数据类型、是否可为空。";
        pub fn name(&self) -> &str { Self::NAME }
        pub fn description(&self) -> &str { Self::DESC }
        pub fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": { "table_name": { "type": "string", "description": "要查看的表名" } },
                "required": ["table_name"]
            })
        }
        pub fn execute(&self, db: &DbEngine, args: serde_json::Value) -> Result<String> {
            let table_name = required_str(&args, "table_name")?;
            if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                anyhow::bail!("非法表名: {}", table_name);
            }
            let result = db.execute_query(&format!("DESCRIBE \"{}\"", table_name))?;
            let mut output = format!("表 {} 的结构：\n", table_name);
            for row in &result.rows {
                let col_name = row.get(0).map(|s| s.as_str()).unwrap_or("?");
                let col_type = row.get(1).map(|s| s.as_str()).unwrap_or("?");
                let nullable = row.get(2).map(|s| s.as_str()).unwrap_or("?");
                output.push_str(&format!("  - {} ({}, {})\n", col_name, col_type, nullable));
            }
            if let Ok(count_result) = db.execute_query(&format!("SELECT COUNT(*) FROM \"{}\"", table_name)) {
                if let Some(row) = count_result.rows.first() {
                    if let Some(cnt) = row.first() {
                        output.push_str(&format!("\n总行数: {}", cnt));
                    }
                }
            }
            Ok(output)
        }
    }

    pub struct ExecuteQueryTool;
    impl ExecuteQueryTool {
        pub const NAME: &'static str = "execute_query";
        pub const DESC: &'static str = "在 DuckDB 上执行 SQL 查询并返回结果。结果最多返回 200 行。";
        pub fn name(&self) -> &str { Self::NAME }
        pub fn description(&self) -> &str { Self::DESC }
        pub fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": { "sql": { "type": "string", "description": "要执行的 DuckDB SQL 语句" } },
                "required": ["sql"]
            })
        }
        pub fn execute(&self, db: &DbEngine, args: serde_json::Value) -> Result<String> {
            let sql = required_str(&args, "sql")?;
            let upper = sql.trim().to_uppercase();
            let forbidden = ["DROP ", "DELETE ", "INSERT ", "UPDATE ", "ALTER ", "CREATE ", "ATTACH ", "DETACH "];
            for kw in &forbidden {
                if upper.starts_with(kw) {
                    anyhow::bail!("安全限制：不允许执行写操作（{}）", kw.trim());
                }
            }
            let result = db.execute_query(&sql)?;
            let mut output = format!("查询完成：{} 行 × {} 列，耗时 {}ms\n\n", result.row_count, result.columns.len(), result.execution_time_ms);
            output.push_str(&format!("| {} |\n", result.columns.join(" | ")));
            output.push_str(&format!("|{}|\n", result.columns.iter().map(|_| "---").collect::<Vec<_>>().join("|")));
            let display_rows = result.rows.len().min(50);
            for row in &result.rows[..display_rows] {
                output.push_str(&format!("| {} |\n", row.join(" | ")));
            }
            if result.rows.len() > 50 {
                output.push_str(&format!("\n... 还有 {} 行未显示", result.rows.len() - 50));
            }
            Ok(output)
        }
    }

    pub struct SampleDataTool;
    impl SampleDataTool {
        pub const NAME: &'static str = "sample_data";
        pub const DESC: &'static str = "查看指定表的前 N 行样本数据。默认返回 10 行。";
        pub fn name(&self) -> &str { Self::NAME }
        pub fn description(&self) -> &str { Self::DESC }
        pub fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "table_name": { "type": "string", "description": "要查看的表名" },
                    "limit": { "type": "integer", "description": "返回的行数，默认 10" }
                },
                "required": ["table_name"]
            })
        }
        pub fn execute(&self, db: &DbEngine, args: serde_json::Value) -> Result<String> {
            let table_name = required_str(&args, "table_name")?;
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10).min(100) as usize;
            if !table_name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                anyhow::bail!("非法表名: {}", table_name);
            }
            let sql = format!("SELECT * FROM \"{}\" LIMIT {}", table_name, limit);
            let result = db.execute_query(&sql)?;
            let mut output = format!("表 {} 的前 {} 行样本：\n\n", table_name, limit);
            output.push_str(&format!("| {} |\n", result.columns.join(" | ")));
            output.push_str(&format!("|{}|\n", result.columns.iter().map(|_| "---").collect::<Vec<_>>().join("|")));
            for row in &result.rows {
                output.push_str(&format!("| {} |\n", row.join(" | ")));
            }
            Ok(output)
        }
    }

    pub struct RepairTableTool;
    impl RepairTableTool {
        pub const NAME: &'static str = "repair_table_schema";
        pub const DESC: &'static str = "修复表结构检测错误。当你发现某个表的列名不对、有大量空行或数据错位时使用此工具。你可以指定标题行位置、数据范围等参数重新加载文件。";
        pub fn name(&self) -> &str { Self::NAME }
        pub fn description(&self) -> &str { Self::DESC }
        pub fn parameters(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "table_name": { "type": "string", "description": "要修复的表名" },
                    "file_path": { "type": "string", "description": "原始文件路径（可从 list_tables 中获取）" },
                    "options": { 
                        "type": "string", 
                        "description": "DuckDB 读取函数的附加参数字符串。例如对于 Excel: 'header=true, range=\'A3:Z100\''；对于 CSV: 'skip=2, delim=\',\''" 
                    }
                },
                "required": ["table_name", "file_path", "options"]
            })
        }
        pub fn execute(&self, db: &DbEngine, args: serde_json::Value) -> Result<String> {
            let table_name = required_str(&args, "table_name")?;
            let file_path = required_str(&args, "file_path")?;
            let options = required_str(&args, "options")?;
            
            db.reload_table(&table_name, &file_path, &options)?;
            
            Ok(format!("表 {} 已成功使用选项 '{}' 重新加载。请通过 describe_table 或 sample_data 验证修复结果。", table_name, options))
        }
    }
}
