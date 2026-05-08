use anyhow::Result;
use futures::StreamExt;
use crate::config::GlobalSettings;
use crate::agent::message::{messages_to_api, AssistantResponse, StreamingToolCall};

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

    /// 带工具的流式 Agent 请求
    pub async fn ask_with_tools_stream(
        &self,
        messages: &[crate::agent::Message],
        tools: &[serde_json::Value],
        on_text: impl Fn(String),
        on_reasoning: impl Fn(String),
        on_tool_call_started: impl Fn(&str, &str, &str),
    ) -> Result<AssistantResponse> {
        let api_messages = messages_to_api(messages);

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": api_messages,
            "stream": true,
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::json!(tools);
            body["tool_choice"] = serde_json::json!("auto");
        }

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

        let mut response_accum = AssistantResponse::default();
        let mut buffer = String::new();

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end().to_string();
                buffer = buffer[pos + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(choice) = parsed["choices"].get(0) {
                            // reasoning_content
                            if let Some(delta) = choice.get("delta") {
                                if let Some(r) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                                    if !r.is_empty() {
                                        on_reasoning(r.to_string());
                                    }
                                }
                                // content
                                if let Some(c) = delta.get("content").and_then(|v| v.as_str()) {
                                    if !c.is_empty() {
                                        response_accum.content =
                                            Some(response_accum.content.unwrap_or_default() + c);
                                        on_text(c.to_string());
                                    }
                                }
                                // tool_calls
                                if let Some(tool_calls_delta) = delta.get("tool_calls") {
                                    if let Some(arr) = tool_calls_delta.as_array() {
                                        for tc_delta in arr {
                                            let index = tc_delta["index"]
                                                .as_u64()
                                                .unwrap_or(0) as usize;

                                            // 确保有足够的 slot
                                            while response_accum.tool_calls.len() <= index {
                                                response_accum.tool_calls.push(StreamingToolCall {
                                                    id: String::new(),
                                                    name: String::new(),
                                                    arguments: String::new(),
                                                });
                                            }
                                            let tc = &mut response_accum.tool_calls[index];

                                            // id（通常在第一个 delta 中出现）
                                            if let Some(id) = tc_delta["id"].as_str() {
                                                tc.id = id.to_string();
                                            }
                                            // function.name
                                            if let Some(name) = tc_delta["function"]["name"].as_str() {
                                                tc.name = name.to_string();
                                            }
                                            // function.arguments（增量拼接）
                                            if let Some(args) = tc_delta["function"]["arguments"].as_str() {
                                                tc.arguments.push_str(args);
                                            }
                                        }
                                    }
                                }
                            }

                            // finish_reason
                            if let Some(finish) = choice.get("finish_reason").and_then(|v| v.as_str()) {
                                if finish == "tool_calls" || finish == "stop" {
                                    // 在完成时通知 UI 每个工具调用
                                    for tc in &response_accum.tool_calls {
                                        let preview: String = tc.arguments.chars().take(200).collect();
                                        let args_preview = if tc.arguments.chars().count() > 200 {
                                            format!("{}...", preview)
                                        } else {
                                            tc.arguments.clone()
                                        };
                                        on_tool_call_started(&tc.id, &tc.name, &args_preview);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(response_accum)
    }
}
