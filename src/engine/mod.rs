use anyhow::Result;
use duckdb::Connection;
use std::path::Path;
use crate::tui::event::{QueryResultData, TableSchema, ColumnInfo};

pub struct DbEngine {
    conn: Connection,
}

impl DbEngine {
    pub fn new() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        // 加载必要的扩展
        conn.execute("INSTALL httpfs; LOAD httpfs;", [])?;
        conn.execute("INSTALL icu; LOAD icu;", [])?;
        conn.execute("INSTALL excel; LOAD excel;", [])?;
        Ok(Self { conn })
    }

    /// 扫描目录并注册数据文件为视图
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
                    
                    let query = match ext.as_str() {
                        "csv" => format!("CREATE OR REPLACE VIEW \"{}\" AS SELECT * FROM read_csv_auto('{}')", table_name, file_path),
                        "parquet" => format!("CREATE OR REPLACE VIEW \"{}\" AS SELECT * FROM read_parquet('{}')", table_name, file_path),
                        "xlsx" | "xls" => format!("CREATE OR REPLACE VIEW \"{}\" AS SELECT * FROM st_read('{}')", table_name, file_path),
                        _ => continue,
                    };

                    self.conn.execute(&query, [])?;
                    
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
