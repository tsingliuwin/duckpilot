use anyhow::Result;
use async_openai::{
    types::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use futures::StreamExt;
use crate::config::GlobalSettings;
use crate::tui::event::TableSchema;

pub struct LlmClient {
    client: Client<async_openai::config::OpenAIConfig>,
    model: String,
}

impl LlmClient {
    pub fn new(settings: &GlobalSettings) -> Self {
        let config = async_openai::config::OpenAIConfig::new()
            .with_api_key(&settings.api_key)
            .with_api_base(&settings.api_base);
        
        let client = Client::with_config(config);
        
        Self {
            client,
            model: settings.model.clone(),
        }
    }

    pub async fn ask_sql_stream(
        &self,
        question: &str,
        schemas: &[TableSchema],
        callback: impl Fn(String),
    ) -> Result<String> {
        let system_prompt = self.build_system_prompt(schemas);
        
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(system_prompt)
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(question)
                    .build()?
                    .into(),
            ])
            .stream(true)
            .build()?;

        let mut stream = self.client.chat().create_stream(request).await?;
        let mut full_content = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    for chat_choice in response.choices {
                        if let Some(ref content) = chat_choice.delta.content {
                            full_content.push_str(content);
                            callback(content.clone());
                        }
                    }
                }
                Err(err) => return Err(anyhow::anyhow!("OpenAI error: {}", err)),
            }
        }

        Ok(full_content)
    }

    fn build_system_prompt(&self, schemas: &[TableSchema]) -> String {
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
