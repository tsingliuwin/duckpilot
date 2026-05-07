use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{GlobalSettings, ProjectConfig};
use crate::engine::DbEngine;
use crate::llm::LlmClient;
use crate::tui::event::{AppEvent, EventHandler, TableSchema};
use crate::tui::mouse::MouseScrollState;
use crate::tui::terminal::TerminalManager;
use crate::tui::widgets::{
    chat::{ChatPanel, MessageRole},
    input::InputBox,
    schema_panel::SchemaPanel,
    status_bar::StatusBarData,
    table_view::TableView,
};

/// 焦点区域
#[derive(Debug, Clone, PartialEq)]
pub enum FocusArea {
    Input,
    Chat,
    Schema,
    Table,
}

/// 右侧视图模式
#[derive(Debug, Clone, PartialEq)]
pub enum ViewMode {
    Chat,
    Table,
    Split,
}

/// 应用核心状态
pub struct App {
    pub running: bool,
    pub focus: FocusArea,
    pub view_mode: ViewMode,
    pub project_dir: PathBuf,

    // UI 组件
    pub chat_panel: ChatPanel,
    pub input_box: InputBox,
    pub schema_panel: SchemaPanel,
    pub table_view: TableView,
    pub status_bar: StatusBarData,

    // 配置
    #[allow(dead_code)]
    pub global_settings: GlobalSettings,
    #[allow(dead_code)]
    pub project_config: ProjectConfig,

    // 引擎与 LLM
    pub engine: Arc<Mutex<DbEngine>>,
    pub llm: Arc<LlmClient>,

    // 缓存 Schema
    pub schemas: Vec<TableSchema>,

    // 事件发送器（用于异步任务回传结果）
    pub event_sender: tokio::sync::mpsc::UnboundedSender<AppEvent>,

    // 面板区域缓存（每次 render 时更新，用于鼠标坐标路由）
    pub chat_panel_area: Option<ratatui::prelude::Rect>,
    pub schema_panel_area: Option<ratatui::prelude::Rect>,
    pub table_view_area: Option<ratatui::prelude::Rect>,
    pub input_box_area: Option<ratatui::prelude::Rect>,

    // 鼠标滚动状态
    mouse_scroll: MouseScrollState,
}

impl App {
    pub fn new(project_dir: PathBuf, event_sender: tokio::sync::mpsc::UnboundedSender<AppEvent>) -> anyhow::Result<Self> {
        let global_settings = GlobalSettings::load().unwrap_or_default();
        let project_config = ProjectConfig::load(&project_dir).unwrap_or_default();

        let engine = Arc::new(Mutex::new(DbEngine::new(&project_dir)?));
        let llm = Arc::new(LlmClient::new(&global_settings));
        let show_reasoning = global_settings.show_reasoning;

        let project_name = if project_config.name.is_empty() {
            project_dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("未知项目")
                .to_string()
        } else {
            project_config.name.clone()
        };

        let status_bar = StatusBarData {
            db_connected: false,
            llm_configured: global_settings.is_configured(),
            model_name: global_settings.model.clone(),
            project_name,
            data_files_count: 0,
        };

        Ok(Self {
            running: true,
            focus: FocusArea::Input,
            view_mode: ViewMode::Chat,
            project_dir,
            chat_panel: {
                let mut panel = ChatPanel::default();
                panel.show_reasoning = show_reasoning;
                panel
            },
            input_box: InputBox::default(),
            schema_panel: SchemaPanel::default(),
            table_view: TableView::default(),
            status_bar,
            global_settings,
            project_config,
            engine,
            llm,
            schemas: Vec::new(),
            event_sender,
            chat_panel_area: None,
            schema_panel_area: None,
            table_view_area: None,
            input_box_area: None,
            mouse_scroll: MouseScrollState::new(),
        })
    }

    /// 启动后台扫描任务
    pub fn start_scanning(&self) {
        let engine = self.engine.clone();
        let project_dir = self.project_dir.clone();
        let tx = self.event_sender.clone();

        tokio::spawn(async move {
            let data_dir = project_dir.join("data");
            let engine_lock = engine.lock().await;
            match engine_lock.scan_and_register_files(&data_dir) {
                Ok(schemas) => {
                    let _ = tx.send(AppEvent::SchemaDone(schemas));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::QueryError(format!("扫描数据文件失败: {}", e)));
                }
            }
        });
    }

    /// 处理事件
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::LlmChunk(chunk) => {
                self.chat_panel.append_streaming(&chunk);
            }
            AppEvent::LlmReasoningChunk(chunk) => {
                self.chat_panel.append_reasoning(&chunk);
            }
            AppEvent::LlmDone => {
                self.chat_panel.finish_streaming();
                
                // 获取刚生成的完整消息
                if let Some(msg) = self.chat_panel.messages.last() {
                    if msg.role == MessageRole::Assistant {
                        let sql = crate::llm::LlmClient::extract_sql(&msg.content);
                        if !sql.is_empty() && !sql.starts_with("--") {
                            // 更新最后一条消息，添加 SQL 显示
                            self.chat_panel.update_last_message_sql(sql.clone());
                            
                            // 执行 SQL
                            self.execute_sql(sql);
                        }
                    }
                }
            }
            AppEvent::LlmError(err) => {
                self.chat_panel.finish_streaming();
                self.chat_panel.add_message(MessageRole::System, format!("❌ LLM 错误: {}", err));
            }
            AppEvent::QueryResult(data) => {
                let summary = format!("查询完成：{} 行 × {} 列，耗时 {}ms", data.row_count, data.columns.len(), data.execution_time_ms);
                self.chat_panel.add_message(MessageRole::System, summary);
                self.table_view.set_data(data);
                self.view_mode = ViewMode::Split;
            }
            AppEvent::QueryError(err) => {
                self.chat_panel.add_message(MessageRole::System, format!("❌ 查询错误: {}", err));
            }
            AppEvent::SchemaDone(schemas) => {
                self.schemas = schemas.clone();
                self.status_bar.data_files_count = schemas.len();
                self.status_bar.db_connected = true;
                self.schema_panel.set_schemas(schemas);
            }
            AppEvent::Resize(_, _) => {
                // 通知 chat 面板视口大小变化，需要重建折行缓存
                self.chat_panel.on_resize();
            }
            AppEvent::Mouse(mouse) => self.handle_mouse(mouse),
            AppEvent::Tick => {}
        }
    }

    fn execute_sql(&self, sql: String) {
        let engine = self.engine.clone();
        let tx = self.event_sender.clone();
        
        tokio::spawn(async move {
            let engine_lock = engine.lock().await;
            match engine_lock.execute_query(&sql) {
                Ok(data) => {
                    let _ = tx.send(AppEvent::QueryResult(data));
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::QueryError(format!("SQL 执行失败: {}", e)));
                }
            }
        });
    }

    fn refresh_data(&mut self) {
        self.chat_panel.add_message(MessageRole::System, "🔄 正在重新扫描 data/ /目录并刷新表结构...".to_string());
        self.start_scanning();
    }

    fn copy_chat_content(&mut self, what: &str) {
        let text = match what {
            "sql" => self.chat_panel.last_sql().map(|s| s.to_string()),
            "reply" => self.chat_panel.last_reply().map(|s| s.to_string()),
            _ => Some(self.chat_panel.full_text()),
        };

        match text {
            Some(content) if !content.is_empty() => {
                match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(&content)) {
                    Ok(()) => {
                        let label = match what {
                            "sql" => "SQL 已复制到剪贴板",
                            "reply" => "最后一条回复已复制到剪贴板",
                            _ => "对话内容已复制到剪贴板",
                        };
                        self.chat_panel.add_message(MessageRole::System, format!("📋 {}", label));
                    }
                    Err(e) => {
                        self.chat_panel.add_message(
                            MessageRole::System,
                            format!("❌ 复制失败: {}", e),
                        );
                    }
                }
            }
            _ => {
                let label = match what {
                    "sql" => "没有可复制的 SQL",
                    "reply" => "没有可复制的回复",
                    _ => "没有可复制的内容",
                };
                self.chat_panel.add_message(MessageRole::System, label.to_string());
            }
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        // 全局快捷键
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                if self.focus != FocusArea::Input {
                    self.focus = FocusArea::Input;
                } else {
                    self.running = false;
                }
                return;
            }
            (KeyCode::Tab, KeyModifiers::NONE) => {
                self.focus = match self.focus {
                    FocusArea::Input => FocusArea::Chat,
                    FocusArea::Chat => FocusArea::Schema,
                    FocusArea::Schema => FocusArea::Table,
                    FocusArea::Table => FocusArea::Input,
                };
                return;
            }
            (KeyCode::BackTab, _) => {
                self.focus = match self.focus {
                    FocusArea::Input => FocusArea::Table,
                    FocusArea::Chat => FocusArea::Input,
                    FocusArea::Schema => FocusArea::Chat,
                    FocusArea::Table => FocusArea::Schema,
                };
                return;
            }
            (KeyCode::F(1), _) => { self.view_mode = ViewMode::Chat; return; }
            (KeyCode::F(2), _) => { self.view_mode = ViewMode::Table; return; }
            (KeyCode::F(3), _) => { self.view_mode = ViewMode::Split; return; }
            (KeyCode::F(5), _) => { self.refresh_data(); return; }
            _ => {}
        }

        // 根据焦点区域分发键盘事件
        match self.focus {
            FocusArea::Input => {
                if let Some(submitted) = self.input_box.handle_key(key) {
                    self.on_submit(submitted);
                }
            }
            FocusArea::Chat => match key.code {
                KeyCode::Up => self.chat_panel.scroll_up(),
                KeyCode::Down => self.chat_panel.scroll_down(),
                KeyCode::PageUp => self.chat_panel.page_up(),
                KeyCode::PageDown => self.chat_panel.page_down(),
                KeyCode::Home => self.chat_panel.scroll_to_top(),
                KeyCode::End => self.chat_panel.scroll_to_end(),
                KeyCode::Char('y') => self.copy_chat_content("all"),
                KeyCode::Char('s') => self.copy_chat_content("sql"),
                KeyCode::Char('r') => self.copy_chat_content("reply"),
                _ => {}
            },
            FocusArea::Schema => match key.code {
                KeyCode::Up => self.schema_panel.previous(),
                KeyCode::Down => self.schema_panel.next(),
                KeyCode::Enter => self.schema_panel.toggle_expand(),
                _ => {}
            },
            FocusArea::Table => match key.code {
                KeyCode::Up => self.table_view.scroll_up(),
                KeyCode::Down => self.table_view.scroll_down(),
                _ => {}
            },
        }
    }

    /// 用户提交输入后的处理
    fn on_submit(&mut self, input: String) {
        // 处理特殊命令
        let trimmed = input.trim();
        if trimmed.starts_with('/') {
            match trimmed {
                "/quit" | "/exit" | "/q" => { self.running = false; return; }
                "/clear" => { self.chat_panel = ChatPanel::default(); return; }
                "/refresh" | "/r" => { self.refresh_data(); return; }
                "/chat" => { self.view_mode = ViewMode::Chat; return; }
                "/table" => { self.view_mode = ViewMode::Table; return; }
                "/split" => { self.view_mode = ViewMode::Split; return; }
                "/help" => {
                    self.chat_panel.add_message(MessageRole::System,
                        "可用命令:\n  /clear - 清空对话\n  /refresh - 刷新数据文件\n  /chat - 聊天视图\n  /table - 表格视图\n  /split - 分屏视图\n  /quit - 退出\n\n快捷键:\n  Tab - 切换焦点\n  F1/F2/F3 - 切换视图\n  F5 - 刷新数据\n  Esc - 退出\n\n对话面板快捷键:\n  ↑/↓ - 逐行滚动\n  PgUp/PgDn - 翻页滚动\n  Home/End - 跳到顶部/底部\n  y - 复制全部对话\n  s - 复制最后一条 SQL\n  r - 复制最后一条回复\n\n🖱️ 鼠标交互:\n  支持原生选择 - 直接用鼠标划选文本即可进行复制".to_string()
                    );
                    return;
                }
                _ => {
                    self.chat_panel.add_message(MessageRole::System, format!("未知命令: {}", trimmed));
                    return;
                }
            }
        }

        // 添加用户消息
        self.chat_panel.add_message(MessageRole::User, input.clone());

        // 发送给 LLM 进行 NL2SQL 处理
        self.chat_panel.start_streaming();
        
        let llm = self.llm.clone();
        let schemas = self.schemas.clone();
        let tx = self.event_sender.clone();
        let question = input;

        tokio::spawn(async move {
            let tx_reasoning = tx.clone();
            let content_callback = |chunk: String| {
                let _ = tx.send(AppEvent::LlmChunk(chunk));
            };
            let reasoning_callback = move |chunk: String| {
                let _ = tx_reasoning.send(AppEvent::LlmReasoningChunk(chunk));
            };

            match llm.ask_sql_stream(&question, &schemas, content_callback, reasoning_callback).await {
                Ok(_) => {
                    let _ = tx.send(AppEvent::LlmDone);
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::LlmError(e.to_string()));
                }
            }
        });
    }

    /// 根据鼠标坐标判断命中的面板，路由滚动事件
    fn panel_at(&self, row: u16, column: u16) -> Option<FocusArea> {
        let areas = [
            (self.schema_panel_area, FocusArea::Schema),
            (self.chat_panel_area, FocusArea::Chat),
            (self.table_view_area, FocusArea::Table),
        ];
        for (area, focus) in &areas {
            if let Some(r) = area {
                if row >= r.y && row < r.y + r.height && column >= r.x && column < r.x + r.width {
                    return Some(focus.clone());
                }
            }
        }
        None
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        let (row, column) = (mouse.row, mouse.column);
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                let delta = self.mouse_scroll.on_scroll(crate::tui::mouse::ScrollDirection::Up);
                if let Some(panel) = self.panel_at(row, column) {
                    match panel {
                        FocusArea::Chat => self.chat_panel.scroll_by(delta),
                        FocusArea::Schema => {
                            if delta < 0 {
                                self.schema_panel.previous();
                            } else {
                                self.schema_panel.next();
                            }
                        }
                        FocusArea::Table => {
                            if delta < 0 {
                                self.table_view.scroll_up();
                            } else {
                                self.table_view.scroll_down();
                            }
                        }
                        _ => {}
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                let delta = self.mouse_scroll.on_scroll(crate::tui::mouse::ScrollDirection::Down);
                if let Some(panel) = self.panel_at(row, column) {
                    match panel {
                        FocusArea::Chat => self.chat_panel.scroll_by(delta),
                        FocusArea::Schema => {
                            if delta < 0 {
                                self.schema_panel.previous();
                            } else {
                                self.schema_panel.next();
                            }
                        }
                        FocusArea::Table => {
                            if delta < 0 {
                                self.table_view.scroll_up();
                            } else {
                                self.table_view.scroll_down();
                            }
                        }
                        _ => {}
                    }
                }
            }
            // 鼠标左键按下：开始选择（或在聊天面板外点击时清除已有选择）
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(FocusArea::Chat) = self.panel_at(row, column) {
                    if let Some(area) = self.chat_panel_area {
                        let inner_y = row.saturating_sub(area.y + 1) as usize;
                        let inner_x = column.saturating_sub(area.x + 1) as usize;
                        self.chat_panel.start_selection(inner_y, inner_x);
                    }
                } else if self.chat_panel.has_selection() {
                    // 点击聊天面板外部，清除选择
                    self.chat_panel.clear_selection();
                }
            }
            // 鼠标拖拽：扩展选择
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.chat_panel.is_dragging() {
                    if let Some(area) = self.chat_panel_area {
                        let inner_y = row.saturating_sub(area.y + 1) as usize;
                        let inner_x = column.saturating_sub(area.x + 1) as usize;
                        self.chat_panel.extend_selection(inner_y, inner_x);
                    }
                }
            }
            // 鼠标松开：结束拖拽，复制到剪贴板，但保留选择可见
            MouseEventKind::Up(MouseButton::Left) => {
                if self.chat_panel.is_dragging() {
                    self.chat_panel.finish_drag();
                    if let Some(text) = self.chat_panel.selected_text() {
                        let _ = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(&text));
                    }
                }
            }
            _ => {}
        }
    }

    /// 运行 TUI 主循环
    pub async fn run(&mut self, events: &mut EventHandler) -> anyhow::Result<()> {
        let mut terminal = TerminalManager::init()?;

        while self.running {
            // 渲染
            terminal.draw(|frame| {
                crate::tui::ui::render(self, frame);
            })?;

            // 等待事件
            if let Some(event) = events.next().await {
                self.handle_event(event);
            }
        }

        TerminalManager::restore()?;
        Ok(())
    }
}
