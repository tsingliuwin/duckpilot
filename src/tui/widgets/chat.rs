use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

/// 聊天消息角色
#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// 聊天消息
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub sql: Option<String>,
    pub timestamp: String,
}

/// 聊天面板状态
#[derive(Debug)]
pub struct ChatPanel {
    pub messages: Vec<ChatMessage>,
    pub scroll_offset: u16,
    pub streaming_text: String,
    pub is_streaming: bool,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            messages: vec![ChatMessage {
                role: MessageRole::System,
                content: "🛸 欢迎使用 DuckPilot！输入自然语言问题开始分析数据。".to_string(),
                sql: None,
                timestamp: chrono::Local::now().format("%H:%M").to_string(),
            }],
            scroll_offset: 0,
            streaming_text: String::new(),
            is_streaming: false,
        }
    }
}

impl ChatPanel {
    pub fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(ChatMessage {
            role,
            content,
            sql: None,
            timestamp: chrono::Local::now().format("%H:%M").to_string(),
        });
        self.scroll_to_bottom();
    }

    pub fn add_message_with_sql(&mut self, role: MessageRole, content: String, sql: String) {
        self.messages.push(ChatMessage {
            role,
            content,
            sql: Some(sql),
            timestamp: chrono::Local::now().format("%H:%M").to_string(),
        });
        self.scroll_to_bottom();
    }

    pub fn start_streaming(&mut self) {
        self.is_streaming = true;
        self.streaming_text.clear();
    }

    pub fn append_streaming(&mut self, chunk: &str) {
        self.streaming_text.push_str(chunk);
    }

    pub fn finish_streaming(&mut self) {
        if !self.streaming_text.is_empty() {
            let content = std::mem::take(&mut self.streaming_text);
            self.add_message(MessageRole::Assistant, content);
        }
        self.is_streaming = false;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// 渲染聊天面板
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" 💬 对话 ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 149, 237)))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // 构建消息文本
        let mut lines: Vec<Line> = Vec::new();

        for msg in &self.messages {
            let (prefix, style) = match msg.role {
                MessageRole::User => (
                    "  🧑 你",
                    Style::default().fg(Color::Rgb(129, 199, 132)).bold(),
                ),
                MessageRole::Assistant => (
                    "  🛸 Pilot",
                    Style::default().fg(Color::Rgb(100, 149, 237)).bold(),
                ),
                MessageRole::System => (
                    "  ⚙️  系统",
                    Style::default().fg(Color::Rgb(255, 183, 77)).bold(),
                ),
            };

            // 角色标签行
            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(
                    format!("  {}", msg.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            // 消息内容
            for line in msg.content.lines() {
                lines.push(Line::from(Span::styled(
                    format!("    {}", line),
                    Style::default().fg(Color::Rgb(220, 220, 220)),
                )));
            }

            // SQL 代码块
            if let Some(sql) = &msg.sql {
                lines.push(Line::from(Span::styled(
                    "    ┌─ SQL ──────────────────",
                    Style::default().fg(Color::Rgb(150, 150, 150)),
                )));
                for sql_line in sql.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("    │ {}", sql_line),
                        Style::default().fg(Color::Rgb(206, 147, 216)),
                    )));
                }
                lines.push(Line::from(Span::styled(
                    "    └───────────────────────",
                    Style::default().fg(Color::Rgb(150, 150, 150)),
                )));
            }

            // 消息间空行
            lines.push(Line::from(""));
        }

        // 流式输出中的文本
        if self.is_streaming {
            lines.push(Line::from(vec![
                Span::styled(
                    "  🛸 Pilot",
                    Style::default().fg(Color::Rgb(100, 149, 237)).bold(),
                ),
                Span::styled("  思考中...", Style::default().fg(Color::DarkGray)),
            ]));
            for line in self.streaming_text.lines() {
                lines.push(Line::from(Span::styled(
                    format!("    {}", line),
                    Style::default().fg(Color::Rgb(220, 220, 220)),
                )));
            }
            // 闪烁光标效果
            lines.push(Line::from(Span::styled(
                "    █",
                Style::default().fg(Color::Rgb(100, 149, 237)),
            )));
        }

        let total_lines = lines.len() as u16;
        let visible_height = inner.height;
        let max_scroll = total_lines.saturating_sub(visible_height);
        let scroll = max_scroll.saturating_sub(self.scroll_offset);

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0));

        frame.render_widget(paragraph, inner);

        // 滚动条
        if total_lines > visible_height {
            let mut scrollbar_state =
                ScrollbarState::new(max_scroll as usize).position(scroll as usize);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("▲"))
                    .end_symbol(Some("▼"))
                    .track_symbol(Some("│"))
                    .thumb_symbol("█"),
                area,
                &mut scrollbar_state,
            );
        }
    }
}
