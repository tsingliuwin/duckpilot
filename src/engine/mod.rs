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
            // 先尝试安装（如果已安装则跳过）
            let _ = conn.execute(&format!("INSTALL {};", ext), []);
            // 尝试加载，如果失败则打印警告但不退出（除了核心功能必需的扩展）
            if let Err(e) = conn.execute(&format!("LOAD {};", ext), []) {
                if ext == "ducklake" {
                    // 如果 ducklake 必需，可以根据需求决定是否报错
                    // 这里我们暂时记录警告，因为普通 DuckDB 也能工作
                    eprintln!("警告: 无法加载 ducklake 扩展: {}", e);
                }
            }
        }

        // 初始化 DuckLake 目录
        let dp_dir = project_dir.join(".duckpilot");
        if !dp_dir.exists() {
            std::fs::create_dir_all(&dp_dir)?;
        }
        
        let lake_path = dp_dir.join("metadata.ducklake");
        let data_path = dp_dir.join("metadata.ducklake.files");
        
        // 挂载 DuckLake Catalog
        // 如果 ducklake 扩展没加载成功，这里的 ATTACH 可能会失败
        let attach_query = format!(
            "ATTACH 'ducklake:{}' AS lake (DATA_PATH '{}')",
            lake_path.to_string_lossy(),
            data_path.to_string_lossy()
        );
        
        match conn.execute(&attach_query, []) {
            Ok(_) => {
                // 默认切换到 lake 数据库
                let _ = conn.execute("USE lake", []);
            },
            Err(e) => {
                eprintln!("警告: 无法挂载 DuckLake 目录: {}", e);
                // 降级使用默认内存数据库
            }
        }

        Ok(Self { conn })
    }

    /// 扫描目录并注册数据文件为 DuckLake 表
    pub fn scan_and_register_files(&self, data_dir: &Path) -> Result<(Vec<TableSchema>, Vec<String>)> {
        let mut schemas = Vec::new();
        let mut warnings = Vec::new();

        if !data_dir.exists() {
            return Ok((schemas, warnings));
        }

        for entry in std::fs::read_dir(data_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(table_name) = path.file_stem().and_then(|s| s.to_str()) {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext = ext.to_lowercase();
                    let file_path = path.to_string_lossy();

                    let warning = match ext.as_str() {
                        "csv" => self.load_csv_as_table(table_name, &file_path)?,
                        "xlsx" | "xls" => self.load_xlsx_as_table(table_name, &file_path)?,
                        "parquet" => {
                            let source_fn = format!("read_parquet('{}')", file_path);
                            self.execute_create_with_source(table_name, &source_fn)?;
                            None
                        }
                        _ => continue,
                    };
                    
                    if let Some(w) = warning {
                        warnings.push(w);
                    }
                    
                    // 获取 Schema
                    let schema = self.get_table_schema(table_name, &file_path)?;
                    schemas.push(schema);
                }
            }
        }

        Ok((schemas, warnings))
    }

    fn get_table_schema(&self, table_name: &str, source_file: &str) -> Result<TableSchema> {
        let mut columns = Vec::new();
        let mut stmt = self.conn.prepare(&format!("DESCRIBE \"{}\"", table_name))?;
        let mut query_rows = stmt.query([])?;
        
        while let Some(row) = query_rows.next()? {
            let name: String = row.get(0)?;
            let data_type: String = row.get(1)?;
            let nullable_str: String = row.get(2)?;
            columns.push(ColumnInfo {
                name,
                data_type,
                nullable: nullable_str == "YES",
                sample_values: Vec::new(),
            });
        }

        Ok(TableSchema {
            name: table_name.to_string(),
            source_file: source_file.to_string(),
            columns,
            row_count: None,
        })
    }

    fn load_xlsx_as_table(&self, table_name: &str, file_path: &str) -> Result<Option<String>> {
        // Strategy 1: Default load
        let default_source = format!("read_xlsx('{}')", file_path);
        if self.try_create_and_validate(table_name, &default_source)? {
            return Ok(None);
        }

        // Strategy 2: Try different header offsets (common in exported reports)
        // Some files have titles in the first few rows.
        // In DuckDB Excel extension, range must be a full range like 'A1:ZZ1000000'
        let offsets = ["A1:ZZ100000", "A2:ZZ100000", "A3:ZZ100000", "A4:ZZ100000", "A5:ZZ100000"];
        for r in offsets {
            let source = format!(
                "read_xlsx('{}', header=true, range='{}', stop_at_empty=false)",
                file_path, r
            );
            if self.try_create_and_validate(table_name, &source)? {
                return Ok(None);
            }
        }

        // Strategy 3: stop_at_empty=false + header + ignore_errors
        let robust_source = format!(
            "read_xlsx('{}', header=true, stop_at_empty=false, ignore_errors=true)",
            file_path
        );
        if self.try_create_and_validate(table_name, &robust_source)? {
            return Ok(None);
        }

        // Strategy 4: all_varchar + stop_at_empty=false
        let varchar_source = format!(
            "read_xlsx('{}', header=true, stop_at_empty=false, all_varchar=true, ignore_errors=true)",
            file_path
        );
        if self.try_create_and_validate(table_name, &varchar_source)? {
            return Ok(None);
        }

        // Fallback: accept best-effort result with warning
        let warning = format!(
            "⚠️ Excel 文件 '{}' 的列检测可能不准确，请检查文件格式（合并单元格/标题行）。你可以尝试使用 /help 查看如何修复。",
            file_path
        );
        self.execute_create_with_source(table_name, &varchar_source)?;
        Ok(Some(warning))
    }

    fn load_csv_as_table(&self, table_name: &str, file_path: &str) -> Result<Option<String>> {
        // Strategy 1: sniff_csv pre-check
        let sniff_ok = self.conn.query_row(
            &format!(
                "SELECT count(column_name) FROM (DESCRIBE (SELECT * FROM sniff_csv('{}')))",
                file_path
            ),
            [],
            |row| row.get::<_, i64>(0),
        ).unwrap_or(0) > 1;

        if sniff_ok {
            let source = format!(
                "read_csv_auto('{}', ignore_errors=true, null_padding=true)",
                file_path
            );
            if self.try_create_and_validate(table_name, &source)? {
                return Ok(None);
            }
        }

        // Strategy 2: full-file scan
        let full_scan = format!(
            "read_csv_auto('{}', sample_size=-1, ignore_errors=true, null_padding=true)",
            file_path
        );
        if self.try_create_and_validate(table_name, &full_scan)? {
            return Ok(None);
        }

        // Strategy 3: try common delimiters
        let delimiters = [";", "\t", "|"];
        for delim in &delimiters {
            let source = format!(
                "read_csv_auto('{}', delim='{}', sample_size=-1, ignore_errors=true, null_padding=true)",
                file_path, delim
            );
            if self.try_create_and_validate(table_name, &source)? {
                return Ok(None);
            }
        }

        // Fallback
        let warning = format!(
            "⚠️ CSV 文件 '{}' 的列检测可能不准确，请检查文件格式",
            file_path
        );
        self.execute_create_with_source(table_name, &full_scan)?;
        Ok(Some(warning))
    }

    fn get_table_column_count(&self, table_name: &str) -> Result<i64> {
        Ok(self.conn.query_row(
            &format!("SELECT count(column_name) FROM (DESCRIBE \"{}\")", table_name),
            [],
            |row| row.get(0),
        )?)
    }

    fn execute_create_with_source(&self, table_name: &str, source_fn: &str) -> Result<()> {
        let query = format!(
            "CREATE TABLE IF NOT EXISTS \"{}\" AS SELECT * FROM {}",
            table_name, source_fn
        );
        match self.conn.execute(&query, []) {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.to_string().contains("already exists") {
                    Ok(())
                } else {
                    Err(e.into())
                }
            }
        }
    }

    fn try_create_and_validate(&self, table_name: &str, source_fn: &str) -> Result<bool> {
        let _ = self.conn.execute(
            &format!("DROP TABLE IF EXISTS \"{}\"", table_name),
            [],
        );
        self.execute_create_with_source(table_name, source_fn)?;
        let col_count = self.get_table_column_count(table_name)?;
        if col_count > 1 {
            Ok(true)
        } else {
            let _ = self.conn.execute(
                &format!("DROP TABLE IF EXISTS \"{}\"", table_name),
                [],
            );
            Ok(false)
        }
    }

    pub fn execute_query(&self, sql: &str) -> Result<QueryResultData> {
        let start = std::time::Instant::now();
        let mut stmt = self.conn.prepare(sql)?;

        // execute() 让 statement 进入 executed 状态，这样才能获取 schema
        stmt.raw_execute()?;
        let schema = stmt.schema();
        let columns: Vec<String> = schema
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect();
        let col_count = columns.len();

        // raw_query() 读取已执行的结果
        let mut query_rows = stmt.raw_query();
        let mut rows: Vec<Vec<String>> = Vec::new();
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

    /// 使用自定义参数重载表。用于 Agent 自主修复数据读取问题。
    pub fn reload_table(&self, table_name: &str, file_path: &str, options: &str) -> Result<()> {
        let ext = Path::new(file_path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let source_fn = match ext.as_str() {
            "xlsx" | "xls" => format!("read_xlsx('{}', {})", file_path, options),
            "csv" => format!("read_csv_auto('{}', {})", file_path, options),
            "parquet" => format!("read_parquet('{}', {})", file_path, options),
            _ => anyhow::bail!("不支持的文件格式: {}", ext),
        };

        // 先删除旧表
        let _ = self.conn.execute(&format!("DROP TABLE IF EXISTS \"{}\"", table_name), []);
        
        // 执行创建
        self.execute_create_with_source(table_name, &source_fn)?;
        Ok(())
    }
}
