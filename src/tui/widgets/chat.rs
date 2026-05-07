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
    pub reasoning: Option<String>,
    pub timestamp: String,
    pub show_reasoning: bool,
}

/// 聊天面板状态
#[derive(Debug)]
pub struct ChatPanel {
    pub messages: Vec<ChatMessage>,
    pub scroll_offset: u16,
    pub streaming_text: String,
    pub streaming_reasoning: String,
    pub is_streaming: bool,
    pub show_reasoning: bool,
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self {
            messages: vec![ChatMessage {
                role: MessageRole::System,
                content: "🛸 欢迎使用 DuckPilot！输入自然语言问题开始分析数据。".to_string(),
                sql: None,
                reasoning: None,
                timestamp: chrono::Local::now().format("%H:%M").to_string(),
                show_reasoning: true,
            }],
            scroll_offset: 0,
            streaming_text: String::new(),
            streaming_reasoning: String::new(),
            is_streaming: false,
            show_reasoning: true,
        }
    }
}

impl ChatPanel {
    pub fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(ChatMessage {
            role,
            content,
            sql: None,
            reasoning: None,
            timestamp: chrono::Local::now().format("%H:%M").to_string(),
            show_reasoning: self.show_reasoning,
        });
        self.scroll_to_bottom();
    }

    pub fn start_streaming(&mut self) {
        self.is_streaming = true;
        self.streaming_text.clear();
        self.streaming_reasoning.clear();
    }

    pub fn append_streaming(&mut self, chunk: &str) {
        self.streaming_text.push_str(chunk);
    }

    pub fn append_reasoning(&mut self, chunk: &str) {
        self.streaming_reasoning.push_str(chunk);
    }

    pub fn finish_streaming(&mut self) {
        let content = std::mem::take(&mut self.streaming_text);
        let reasoning = std::mem::take(&mut self.streaming_reasoning);
        if !content.is_empty() || !reasoning.is_empty() {
            self.messages.push(ChatMessage {
                role: MessageRole::Assistant,
                content,
                sql: None,
                reasoning: if reasoning.is_empty() { None } else { Some(reasoning) },
                timestamp: chrono::Local::now().format("%H:%M").to_string(),
                show_reasoning: self.show_reasoning,
            });
            self.scroll_to_bottom();
        }
        self.is_streaming = false;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn page_up(&mut self, page_size: u16) {
        self.scroll_offset = self.scroll_offset.saturating_add(page_size);
    }

    pub fn page_down(&mut self, page_size: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
    }

    pub fn scroll_to_top(&mut self) {
        // 设一个很大的值，render 时会被 max_scroll 钳位
        self.scroll_offset = u16::MAX;
    }

    pub fn scroll_to_end(&mut self) {
        self.scroll_offset = 0;
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// 生成全部对话的纯文本
    pub fn full_text(&self) -> String {
        let mut out = String::new();
        for msg in &self.messages {
            let role = match msg.role {
                MessageRole::User => "你",
                MessageRole::Assistant => "Pilot",
                MessageRole::System => "系统",
            };
            out.push_str(&format!("[{}] {}\n{}\n", role, msg.timestamp, msg.content));
            if let Some(sql) = &msg.sql {
                out.push_str(&format!("SQL:\n{}\n", sql));
            }
            out.push('\n');
        }
        out
    }

    /// 获取最后一条包含 SQL 的消息中的 SQL
    pub fn last_sql(&self) -> Option<&str> {
        self.messages.iter().rev().find_map(|m| m.sql.as_deref())
    }

    /// 获取最后一条 Assistant 消息的纯文本
    pub fn last_reply(&self) -> Option<&str> {
        self.messages.iter().rev().find_map(|m| {
            if m.role == MessageRole::Assistant {
                Some(m.content.as_str())
            } else {
                None
            }
        })
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

            // 推理过程
            if let Some(reasoning) = &msg.reasoning {
                if msg.show_reasoning && !reasoning.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    ┌─ 💭 思考 ──────────────────",
                        Style::default().fg(Color::Rgb(100, 100, 120)),
                    )));
                    for rline in reasoning.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("    │ {}", rline),
                            Style::default().fg(Color::Rgb(120, 120, 150)),
                        )));
                    }
                    lines.push(Line::from(Span::styled(
                        "    └───────────────────────────",
                        Style::default().fg(Color::Rgb(100, 100, 120)),
                    )));
                } else if !reasoning.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    💭 思考过程已隐藏",
                        Style::default().fg(Color::Rgb(80, 80, 100)),
                    )));
                }
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
            // 流式推理过程
            if !self.streaming_reasoning.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    ┌─ 💭 思考 ──────────────────",
                    Style::default().fg(Color::Rgb(100, 100, 120)),
                )));
                for rline in self.streaming_reasoning.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("    │ {}", rline),
                        Style::default().fg(Color::Rgb(120, 120, 150)),
                    )));
                }
            }
            // 流式内容
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

        let total_lines = count_visual_lines(&lines, inner.width);
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

/// 计算考虑文本换行后的实际可视行数
fn count_visual_lines(lines: &[Line], width: u16) -> u16 {
    if width == 0 {
        return lines.len() as u16;
    }
    let mut total: u16 = 0;
    for line in lines {
        let line_width = line.width() as u16;
        total = total.saturating_add(if line_width == 0 {
            1
        } else {
            (line_width + width - 1) / width
        });
    }
    total
}
