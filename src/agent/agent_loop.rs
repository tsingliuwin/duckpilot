use std::sync::Arc;
use tokio::sync::Mutex;

use crate::engine::DbEngine;
use crate::llm::LlmClient;
use crate::tui::event::AppEvent;
use crate::agent::message::Message;
use crate::agent::tool::ToolRegistry;

fn build_system_prompt() -> String {
    String::from(
        "你是一个专业的数据分析助手。你可以使用工具来查询和分析 DuckDB 数据库中的数据。\n\n\
         工作方式：\n\
         1. 先用 list_tables 了解有哪些数据\n\
         2. 用 describe_table 查看感兴趣的表结构\n\
         3. 用 sample_data 查看样本数据\n\
         4. 用 execute_query 执行 SQL 查询进行分析\n\n\
         回答规则：\n\
         - 先思考需要做什么，再选择合适的工具\n\
         - SQL 查询结果可能很大，优先用聚合和统计\n\
         - 用中文回答用户的问题\n\
         - 如果用户的问题不需要查询数据，直接回答即可\n\n\
         重要：当你已经收集到足够的信息来回答用户问题时，必须停止调用工具，直接给出最终回答。不要重复查询相同的数据。"
    )
}

pub async fn run_agent_loop(
    question: String,
    llm: Arc<LlmClient>,
    db: Arc<Mutex<DbEngine>>,
    registry: ToolRegistry,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
    let tools = registry.to_api_tools();
    let mut messages = vec![Message::System {
        content: build_system_prompt(),
    }];
    messages.push(Message::User {
        content: question,
    });

    let max_steps = 15u32;
    let mut step = 0u32;
    let mut last_tool_signature: Option<String> = None;

    loop {
        step += 1;
        if step > max_steps {
            let _ = tx.send(AppEvent::LlmError(format!(
                "达到最大步数限制 ({})，请简化问题或分步提问", max_steps
            )));
            return;
        }

        let tx_text = tx.clone();
        let tx_reasoning = tx.clone();
        let tx_tool_started = tx.clone();

        let result = llm
            .ask_with_tools_stream(
                &messages,
                &tools,
                move |chunk| {
                    let _ = tx_text.send(AppEvent::LlmChunk(chunk));
                },
                move |chunk| {
                    let _ = tx_reasoning.send(AppEvent::LlmReasoningChunk(chunk));
                },
                move |id, name, args| {
                    let _ = tx_tool_started.send(AppEvent::ToolCallStarted {
                        id: id.to_string(),
                        name: name.to_string(),
                        args: args.to_string(),
                    });
                },
            )
            .await;

        let response = match result {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(AppEvent::LlmError(e.to_string()));
                return;
            }
        };

        let has_tool_calls = response.has_tool_calls();
        messages.push(response.into_message());

        if !has_tool_calls {
            let _ = tx.send(AppEvent::LlmDone);
            return;
        }

        // 有工具调用：先结束当前流式状态，把已接收的文本落盘为消息
        let _ = tx.send(AppEvent::LlmDone);

        let tool_calls = match messages.iter().rev().find(|m| matches!(m, Message::Assistant { tool_calls: Some(_), .. })) {
            Some(Message::Assistant { tool_calls: Some(tcs), .. }) => tcs.clone(),
            _ => {
                return;
            }
        };

        // 重复检测：如果工具调用签名和上次完全相同，注入警告
        let sig: String = tool_calls.iter()
            .map(|tc| format!("{}:{}", tc.name, tc.arguments))
            .collect::<Vec<_>>()
            .join("|");
        if Some(&sig) == last_tool_signature.as_ref() {
            messages.push(Message::System {
                content: "系统提示：你正在重复相同的工具调用。请根据已有数据直接给出最终回答，不要再调用工具。".to_string(),
            });
        }
        last_tool_signature = Some(sig);

        // Step nudge：步数较多时提醒 LLM 收尾
        if step >= 6 {
            messages.push(Message::System {
                content: format!("系统提示：已执行 {} 步操作。请综合已有结果，直接给出最终回答。", step),
            });
        }

        let db_lock = db.lock().await;
        for tc in &tool_calls {
            let (content, is_error) = match registry.get(&tc.name) {
                Some(tool) => {
                    match tool.execute(&db_lock, tc.arguments.clone()) {
                        Ok(output) => {
                            let _ = tx.send(AppEvent::ToolCallResult {
                                id: tc.id.clone(),
                                name: tc.name.clone(),
                                result: output.clone(),
                                is_error: false,
                            });
                            (output, false)
                        }
                        Err(e) => {
                            let err_msg = format!("工具执行失败: {}", e);
                            let _ = tx.send(AppEvent::ToolCallResult {
                                id: tc.id.clone(),
                                name: tc.name.clone(),
                                result: err_msg.clone(),
                                is_error: true,
                            });
                            (err_msg, true)
                        }
                    }
                }
                None => {
                    let err_msg = format!("未知工具: {}", tc.name);
                    let _ = tx.send(AppEvent::ToolCallResult {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        result: err_msg.clone(),
                        is_error: true,
                    });
                    (err_msg, true)
                }
            };

            messages.push(Message::ToolResult {
                tool_call_id: tc.id.clone(),
                name: tc.name.clone(),
                content,
                is_error,
            });
        }
        drop(db_lock);

        // 重新启动流式状态，准备接收下一轮 LLM 响应
        let _ = tx.send(AppEvent::LlmStreamStart);
    }
}
