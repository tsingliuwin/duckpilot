use anyhow::Result;
use duckdb::Connection;
use std::path::Path;
use crate::tui::event::{QueryResultData, TableSchema, ColumnInfo};

pub struct DbEngine {
    conn: Connection,
}

impl DbEngine {
    pub fn new(project_dir: &Path) -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        // 加载必要的扩展
        let extensions = vec!["httpfs", "icu", "spatial", "excel", "ducklake"];
        for ext in extensions {
            // 先尝试安装（如果已安装则跳过），然后加载
            let _ = conn.execute(&format!("INSTALL {};", ext), []);
            conn.execute(&format!("LOAD {};", ext), [])?;
        }

        // 初始化 DuckLake 目录
        let dp_dir = project_dir.join(".duckpilot");
        let lake_path = dp_dir.join("metadata.ducklake");
        let data_path = dp_dir.join("metadata.ducklake.files");
        
        // 挂载 DuckLake Catalog
        // 使用 TYPE ducklake 确保以 DuckLake 格式挂载
        let attach_query = format!(
            "ATTACH 'ducklake:{}' AS lake (DATA_PATH '{}')",
            lake_path.to_string_lossy(),
            data_path.to_string_lossy()
        );
        conn.execute(&attach_query, [])?;
        
        // 默认切换到 lake 数据库，这样创建的表都会在 DuckLake 中
        conn.execute("USE lake", [])?;

        Ok(Self { conn })
    }

    /// 扫描目录并注册数据文件为 DuckLake 表
    pub fn scan_and_register_files(&self, data_dir: &Path) -> Result<Vec<TableSchema>> {
        let mut schemas = Vec::new();

        if !data_dir.exists() {
            return Ok(schemas);
        }

        for entry in std::fs::read_dir(data_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(table_name) = path.file_stem().and_then(|s| s.to_str()) {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext = ext.to_lowercase();
                    let file_path = path.to_string_lossy();
                    
                    let source_fn = match ext.as_str() {
                        "csv" => format!("read_csv_auto('{}')", file_path),
                        "parquet" => format!("read_parquet('{}')", file_path),
                        "xlsx" => format!("read_xlsx('{}')", file_path),
                        "xls" => format!("read_xlsx('{}')", file_path),
                        _ => continue,
                    };

                    // 将各种格式的数据转换为 DuckLake 表
                    // 使用 CREATE TABLE IF NOT EXISTS ... AS SELECT * FROM ...
                    let query = format!("CREATE TABLE IF NOT EXISTS \"{}\" AS SELECT * FROM {}", table_name, source_fn);

                    match self.conn.execute(&query, []) {
                        Ok(_) => {},
                        Err(e) => {
                            // 如果表已存在但结构不同，可能需要处理，这里暂时跳过
                            if !e.to_string().contains("already exists") {
                                return Err(e.into());
                            }
                        }
                    }
                    
                    // 获取 Schema
                    let schema = self.get_table_schema(table_name, &file_path)?;
                    schemas.push(schema);
                }
            }
        }

        Ok(schemas)
    }

    fn get_table_schema(&self, table_name: &str, source_file: &str) -> Result<TableSchema> {
        let mut stmt = self.conn.prepare(&format!("DESCRIBE \"{}\"", table_name))?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let data_type: String = row.get(1)?;
            let nullable_str: String = row.get(2)?;
            Ok(ColumnInfo {
                name,
                data_type,
                nullable: nullable_str == "YES",
                sample_values: Vec::new(), // 暂时不获取样本值
            })
        })?;

        let mut columns = Vec::new();
        for row in rows {
            columns.push(row?);
        }

        // 获取行数
        let row_count: usize = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM \"{}\"", table_name),
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        Ok(TableSchema {
            name: table_name.to_string(),
            source_file: source_file.to_string(),
            columns,
            row_count: Some(row_count),
        })
    }

    pub fn execute_query(&self, sql: &str) -> Result<QueryResultData> {
        let start = std::time::Instant::now();
        let mut stmt = self.conn.prepare(sql)?;
        let col_count = stmt.column_count();
        let mut columns = Vec::new();
        for i in 0..col_count {
            columns.push(stmt.column_name(i)?.to_string());
        }

        let mut rows = Vec::new();
        let mut query_rows = stmt.query([])?;
        while let Some(row) = query_rows.next()? {
            let mut row_data = Vec::new();
            for i in 0..col_count {
                let val: duckdb::types::Value = row.get(i)?;
                let val_str = match val {
                    duckdb::types::Value::Null => "NULL".to_string(),
                    duckdb::types::Value::Boolean(b) => b.to_string(),
                    duckdb::types::Value::TinyInt(i) => i.to_string(),
                    duckdb::types::Value::SmallInt(i) => i.to_string(),
                    duckdb::types::Value::Int(i) => i.to_string(),
                    duckdb::types::Value::BigInt(i) => i.to_string(),
                    duckdb::types::Value::HugeInt(i) => i.to_string(),
                    duckdb::types::Value::Float(f) => f.to_string(),
                    duckdb::types::Value::Double(f) => f.to_string(),
                    duckdb::types::Value::Text(t) => t,
                    duckdb::types::Value::Blob(b) => format!("<blob {} bytes>", b.len()),
                    _ => format!("{:?}", val),
                };
                row_data.push(val_str);
            }
            rows.push(row_data);
        }

        let execution_time_ms = start.elapsed().as_millis() as u64;
        let row_count = rows.len();

        Ok(QueryResultData {
            columns,
            rows,
            row_count,
            execution_time_ms,
        })
    }
}
