use serde::{Deserialize, Serialize};

/// 一次工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Agent 会话中的结构化消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "system")]
    System {
        content: String,
    },
    #[serde(rename = "user")]
    User {
        content: String,
    },
    #[serde(rename = "assistant")]
    Assistant {
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCall>>,
    },
    #[serde(rename = "tool")]
    ToolResult {
        #[serde(rename = "tool_call_id")]
        tool_call_id: String,
        name: String,
        content: String,
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
}

/// LLM 流式响应中累积的 assistant 响应
#[derive(Debug, Default)]
pub struct AssistantResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<StreamingToolCall>,
}

/// 流式接收中的工具调用（arguments 增量拼接）
#[derive(Debug, Clone)]
pub struct StreamingToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

impl StreamingToolCall {
    /// 尝试将累积的 arguments 解析为 JSON Value
    pub fn parse_arguments(&self) -> serde_json::Value {
        serde_json::from_str(&self.arguments).unwrap_or(serde_json::Value::Null)
    }
}

impl AssistantResponse {
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    /// 转换为 Message（用于追加到会话历史）
    pub fn into_message(self) -> Message {
        let tool_calls = if self.tool_calls.is_empty() {
            None
        } else {
            Some(
                self.tool_calls
                    .into_iter()
                    .map(|tc| {
                        let args = tc.parse_arguments();
                        ToolCall {
                            id: tc.id,
                            name: tc.name,
                            arguments: args,
                        }
                    })
                    .collect(),
            )
        };
        Message::Assistant {
            content: self.content,
            tool_calls,
        }
    }
}

/// 将消息历史序列化为 OpenAI API 格式的 JSON 数组
pub fn messages_to_api(messages: &[Message]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|msg| match msg {
            Message::System { content } => serde_json::json!({
                "role": "system",
                "content": content,
            }),
            Message::User { content } => serde_json::json!({
                "role": "user",
                "content": content,
            }),
            Message::Assistant {
                content,
                tool_calls,
            } => {
                let mut v = serde_json::json!({
                    "role": "assistant",
                });
                if let Some(c) = content {
                    v["content"] = serde_json::json!(c);
                } else {
                    v["content"] = serde_json::Value::Null;
                }
                if let Some(tcs) = tool_calls {
                    v["tool_calls"] = serde_json::json!(tcs.iter().map(|tc| {
                        serde_json::json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": tc.arguments.to_string(),
                            }
                        })
                    }).collect::<Vec<_>>());
                }
                v
            }
            Message::ToolResult {
                tool_call_id,
                name,
                content,
                is_error,
            } => {
                let mut v = serde_json::json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": content,
                });
                if *is_error {
                    v["is_error"] = serde_json::json!(true);
                }
                let _ = name;
                v
            }
        })
        .collect()
}
