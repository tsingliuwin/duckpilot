use serde::{Deserialize, Serialize};

/// SQL 查询结果数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResultData {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub execution_time_ms: u64,
}

/// 表结构信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub source_file: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<usize>,
}

/// 列详细信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub sample_values: Vec<String>,
}
