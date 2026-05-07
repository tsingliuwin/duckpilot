use anyhow::Result;
use futures::StreamExt;
use crate::config::GlobalSettings;
use crate::models::TableSchema;

pub struct LlmClient {
    model: String,
    api_key: String,
    api_base: String,
}

impl LlmClient {
    pub fn new(settings: &GlobalSettings) -> Self {
        Self {
            model: settings.model.clone(),
            api_key: settings.api_key.clone(),
            api_base: settings.api_base.clone(),
        }
    }

    /// 流式请求 LLM，分别回调 content 和 reasoning_content
    pub async fn ask_sql_stream(
        &self,
        question: &str,
        schemas: &[TableSchema],
        content_callback: impl Fn(String),
        reasoning_callback: impl Fn(String),
    ) -> Result<(String, String)> {
        let system_prompt = Self::build_system_prompt(schemas);

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": question}
            ],
            "stream": true
        });

        let url = format!("{}/chat/completions", self.api_base.trim_end_matches('/'));

        let response = reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("API error {}: {}", status, text);
        }

        let mut full_content = String::new();
        let mut full_reasoning = String::new();
        let mut buffer = String::new();

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end().to_string();
                buffer = buffer[pos + 1..].to_string();

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(delta) = parsed["choices"].get(0).and_then(|c| c.get("delta")) {
                            if let Some(r) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                                if !r.is_empty() {
                                    full_reasoning.push_str(r);
                                    reasoning_callback(r.to_string());
                                }
                            }
                            if let Some(c) = delta.get("content").and_then(|v| v.as_str()) {
                                if !c.is_empty() {
                                    full_content.push_str(c);
                                    content_callback(c.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok((full_content, full_reasoning))
    }

    fn build_system_prompt(schemas: &[TableSchema]) -> String {
        let mut prompt = String::from(
            "你是一个专业的 SQL 生成助手，专门为 DuckDB 编写 SQL。\n\
            请根据提供的表结构，将用户的自然语言问题转换为合法的 DuckDB SQL 语句。\n\n\
            约束条件：\n\
            1. 只输出 SQL 语句，不要包含任何解释文字。\n\
            2. 确保 SQL 语法符合 DuckDB 要求。\n\
            3. 如果用户的问题无法转换为 SQL，请返回一个说明性的错误提示（以 -- 开头）。\n\n\
            当前表结构如下：\n"
        );

        for schema in schemas {
            prompt.push_str(&format!("\n表名: {}\n", schema.name));
            prompt.push_str("列信息:\n");
            for col in &schema.columns {
                prompt.push_str(&format!("  - {} ({}, {})\n", col.name, col.data_type, if col.nullable { "可为空" } else { "必填" }));
            }
        }

        prompt
    }

    /// 提取 SQL 语句（如果 LLM 返回了 Markdown 代码块）
    pub fn extract_sql(content: &str) -> String {
        if let Some(start) = content.find("```sql") {
            if let Some(end) = content[start + 6..].find("```") {
                return content[start + 6..start + 6 + end].trim().to_string();
            }
        } else if let Some(start) = content.find("```") {
             if let Some(end) = content[start + 3..].find("```") {
                return content[start + 3..start + 3 + end].trim().to_string();
            }
        }
        content.trim().to_string()
    }
}
