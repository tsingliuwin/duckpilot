use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;

/// 应用事件类型
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppEvent {
    /// 键盘事件
    Key(KeyEvent),
    /// 鼠标事件
    Mouse(MouseEvent),
    /// 终端大小变化
    Resize(u16, u16),
    /// 定时 tick（用于动画/定期更新）
    Tick,
    /// LLM 流式输出片段
    LlmChunk(String),
    /// LLM 推理思考片段
    LlmReasoningChunk(String),
    /// LLM 输出完成
    LlmDone,
    /// Agent 重新开始一轮 LLM 流式输出
    LlmStreamStart,
    /// LLM 错误
    LlmError(String),
    /// 查询结果
    QueryResult(QueryResultData),
    /// 查询错误
    QueryError(String),
    /// Schema 扫描完成
    SchemaDone(Vec<TableSchema>),
    /// Agent 工具调用开始
    ToolCallStarted { id: String, name: String, args: String },
    /// Agent 工具调用结果
    ToolCallResult { id: String, name: String, result: String, is_error: bool },
}

/// 查询结果数据
#[derive(Debug, Clone)]
pub struct QueryResultData {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
    pub execution_time_ms: u64,
}

/// 表 Schema 信息
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TableSchema {
    pub name: String,
    pub source_file: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: Option<usize>,
}

/// 列信息
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub sample_values: Vec<String>,
}

/// 事件处理器
pub struct EventHandler {
    /// 事件发送端（用于外部发送自定义事件）
    pub sender: mpsc::UnboundedSender<AppEvent>,
    /// 事件接收端
    receiver: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventHandler {
    /// 创建新的事件处理器并启动后台事件监听
    pub fn new(tick_rate: Duration) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        let tx = sender.clone();
        tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick_interval = tokio::time::interval(tick_rate);

            loop {
                tokio::select! {
                    // 处理终端事件
                    Some(Ok(evt)) = reader.next() => {
                        let app_event = match evt {
                            CrosstermEvent::Key(key) => {
                                // 只处理 Press 事件，忽略 Release/Repeat
                                if key.kind == event::KeyEventKind::Press {
                                    Some(AppEvent::Key(key))
                                } else {
                                    None
                                }
                            }
                            CrosstermEvent::Mouse(mouse) => Some(AppEvent::Mouse(mouse)),
                            CrosstermEvent::Resize(w, h) => Some(AppEvent::Resize(w, h)),
                            _ => None,
                        };
                        if let Some(event) = app_event {
                            if tx.send(event).is_err() {
                                break;
                            }
                        }
                    }
                    // Tick 事件
                    _ = tick_interval.tick() => {
                        if tx.send(AppEvent::Tick).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Self { sender, receiver }
    }

    /// 异步等待下一个事件
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.receiver.recv().await
    }
}
