use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};
use unicode_width::UnicodeWidthStr;

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

/// 文本选择端点（视口内坐标）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectionPoint {
    line: usize,
    col: usize,
}

/// 文本选择状态
#[derive(Debug, Clone, Default)]
pub struct Selection {
    anchor: Option<SelectionPoint>,
    head: Option<SelectionPoint>,
    dragging: bool,
}

impl Selection {
    fn clear(&mut self) {
        self.anchor = None;
        self.head = None;
        self.dragging = false;
    }

    fn is_active(&self) -> bool {
        self.anchor.is_some() && self.head.is_some()
    }

    /// 返回有序的 (start, end) 端点
    fn ordered(&self) -> Option<(SelectionPoint, SelectionPoint)> {
        let a = self.anchor?;
        let h = self.head?;
        if (h.line, h.col) < (a.line, a.col) {
            Some((h, a))
        } else {
            Some((a, h))
        }
    }

    fn has_content(&self) -> bool {
        self.ordered().is_some_and(|(s, e)| s != e)
    }
}

/// 聊天面板状态
///
/// 使用正向滚动模型：
/// - `scroll_position = 0` → 显示最顶部
/// - `scroll_position = max_scroll` → 显示最底部
/// - `auto_follow = true` → 新消息到达时自动跳到底部
#[derive(Debug)]
pub struct ChatPanel {
    pub messages: Vec<ChatMessage>,
    /// 正向滚动位置：视口顶部对应的行号
    scroll_position: usize,
    /// 是否自动跟随底部（新消息到达时自动滚到最新）
    auto_follow: bool,
    /// 上次渲染时缓存的视口高度
    pub visible_height: u16,
    /// 上次渲染时的视口宽度（用于检测是否需要重建折行缓存）
    last_width: u16,
    /// 预折行后的全部行（缓存）
    cached_lines: Vec<Line<'static>>,
    /// 每行对应的纯文本（用于选择复制）
    plain_lines: Vec<String>,
    /// 内容是否发生了变化，需要重建缓存
    lines_dirty: bool,

    pub streaming_text: String,
    pub streaming_reasoning: String,
    pub is_streaming: bool,
    pub show_reasoning: bool,

    /// 文本选择状态
    selection: Selection,
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
            scroll_position: 0,
            auto_follow: true,
            visible_height: 0,
            last_width: 0,
            cached_lines: Vec::new(),
            plain_lines: Vec::new(),
            lines_dirty: true,
            streaming_text: String::new(),
            streaming_reasoning: String::new(),
            is_streaming: false,
            show_reasoning: true,
            selection: Selection::default(),
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
        self.lines_dirty = true;
        // auto_follow 模式下，新消息自动跳底部
        if self.auto_follow {
            self.scroll_to_end();
        }
    }

    pub fn start_streaming(&mut self) {
        self.is_streaming = true;
        self.streaming_text.clear();
        self.streaming_reasoning.clear();
        self.lines_dirty = true;
    }

    pub fn append_streaming(&mut self, chunk: &str) {
        self.streaming_text.push_str(chunk);
        self.lines_dirty = true;
    }

    pub fn append_reasoning(&mut self, chunk: &str) {
        self.streaming_reasoning.push_str(chunk);
        self.lines_dirty = true;
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
        }
        self.is_streaming = false;
        self.lines_dirty = true;
        if self.auto_follow {
            self.scroll_to_end();
        }
    }

    /// 更新最后一条消息的 SQL
    pub fn update_last_message_sql(&mut self, sql: String) {
        if let Some(msg) = self.messages.last_mut() {
            msg.sql = Some(sql);
            self.lines_dirty = true;
        }
    }

    // ---- 滚动操作 ----

    /// 统一的滚动入口：delta > 0 向下（看更新的），delta < 0 向上（看更旧的）
    pub fn scroll_by(&mut self, delta: i32) {
        let max_scroll = self.max_scroll();
        let new_pos = (self.scroll_position as i64 + delta as i64)
            .max(0)
            .min(max_scroll as i64) as usize;
        self.scroll_position = new_pos;

        // 如果滚到底部，重新启用 auto_follow
        if self.scroll_position >= max_scroll {
            self.auto_follow = true;
        } else {
            self.auto_follow = false;
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll_by(-1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_by(1);
    }

    pub fn page_up(&mut self) {
        let page = self.visible_height.max(1) as i32;
        self.scroll_by(-page);
    }

    pub fn page_down(&mut self) {
        let page = self.visible_height.max(1) as i32;
        self.scroll_by(page);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_position = 0;
        self.auto_follow = false;
    }

    pub fn scroll_to_end(&mut self) {
        self.scroll_position = self.max_scroll();
        self.auto_follow = true;
    }

    /// 当前最大可滚动行数
    fn max_scroll(&self) -> usize {
        self.cached_lines.len().saturating_sub(self.visible_height as usize)
    }

    // ---- 文本选择 ----

    pub fn start_selection(&mut self, line: usize, col: usize) {
        let pt = SelectionPoint { line, col };
        self.selection.anchor = Some(pt);
        self.selection.head = Some(pt);
        self.selection.dragging = true;
    }

    pub fn extend_selection(&mut self, line: usize, col: usize) {
        if self.selection.anchor.is_some() {
            self.selection.head = Some(SelectionPoint { line, col });
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    pub fn has_selection(&self) -> bool {
        self.selection.is_active()
    }

    pub fn is_dragging(&self) -> bool {
        self.selection.dragging
    }

    /// 结束拖拽状态，保留选择可见
    pub fn finish_drag(&mut self) {
        self.selection.dragging = false;
    }

    /// 将显示列号转换为字符串的字符索引（安全处理多字节字符）
    fn display_col_to_char_idx(s: &str, col: usize) -> usize {
        let mut width = 0usize;
        for (i, ch) in s.char_indices() {
            if width >= col {
                return i;
            }
            width += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1);
        }
        s.len()
    }

    /// 提取选中文本的纯文本内容
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.selection.ordered()?;
        if start.line == end.line && start.col == end.col {
            return None;
        }
        let mut out = String::new();
        for i in start.line..=end.line.min(self.plain_lines.len().saturating_sub(1)) {
            let line = &self.plain_lines[i];
            if start.line == end.line && i == start.line {
                let s = Self::display_col_to_char_idx(line, start.col);
                let e = Self::display_col_to_char_idx(line, end.col);
                if s < e {
                    out.push_str(&line[s..e]);
                }
            } else if i == start.line {
                let s = Self::display_col_to_char_idx(line, start.col);
                out.push_str(&line[s..]);
                out.push('\n');
            } else if i == end.line {
                let e = Self::display_col_to_char_idx(line, end.col);
                out.push_str(&line[..e]);
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        if out.is_empty() { None } else { Some(out) }
    }

    // ---- 内容访问 ----

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

    // ---- 预折行 ----

    /// 当视口宽度变化或内容变化时，重建预折行缓存
    fn rebuild_lines(&mut self, width: u16) {
        let w = width as usize;
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut plain: Vec<String> = Vec::new();

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
            let label = format!("{}  {}", prefix, msg.timestamp);
            lines.push(Line::from(vec![
                Span::styled(prefix.to_string(), style),
                Span::styled(
                    format!("  {}", msg.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            plain.push(label);

            // 消息内容（预折行）
            let content_style = Style::default().fg(Color::Rgb(220, 220, 220));
            for line in msg.content.lines() {
                let prefixed = format!("    {}", line);
                wrap_line_into(&prefixed, w, content_style, &mut lines, &mut plain);
            }

            // 推理过程
            if let Some(reasoning) = &msg.reasoning {
                if msg.show_reasoning && !reasoning.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    ┌─ 💭 思考 ──────────────────".to_string(),
                        Style::default().fg(Color::Rgb(100, 100, 120)),
                    )));
                    plain.push("    ┌─ 💭 思考 ──────────────────".to_string());
                    let r_style = Style::default().fg(Color::Rgb(120, 120, 150));
                    for rline in reasoning.lines() {
                        let prefixed = format!("    │ {}", rline);
                        wrap_line_into(&prefixed, w, r_style, &mut lines, &mut plain);
                    }
                    lines.push(Line::from(Span::styled(
                        "    └───────────────────────────".to_string(),
                        Style::default().fg(Color::Rgb(100, 100, 120)),
                    )));
                    plain.push("    └───────────────────────────".to_string());
                } else if !reasoning.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    💭 思考过程已隐藏".to_string(),
                        Style::default().fg(Color::Rgb(80, 80, 100)),
                    )));
                    plain.push("    💭 思考过程已隐藏".to_string());
                }
            }

            // SQL 代码块
            if let Some(sql) = &msg.sql {
                lines.push(Line::from(Span::styled(
                    "    ┌─ SQL ──────────────────".to_string(),
                    Style::default().fg(Color::Rgb(150, 150, 150)),
                )));
                plain.push("    ┌─ SQL ──────────────────".to_string());
                let sql_style = Style::default().fg(Color::Rgb(206, 147, 216));
                for sql_line in sql.lines() {
                    let prefixed = format!("    │ {}", sql_line);
                    wrap_line_into(&prefixed, w, sql_style, &mut lines, &mut plain);
                }
                lines.push(Line::from(Span::styled(
                    "    └───────────────────────".to_string(),
                    Style::default().fg(Color::Rgb(150, 150, 150)),
                )));
                plain.push("    └───────────────────────".to_string());
            }

            // 消息间空行
            lines.push(Line::from("".to_string()));
            plain.push(String::new());
        }

        // 流式输出中的文本
        if self.is_streaming {
            let status = if self.streaming_text.is_empty() {
                "  思考中..."
            } else {
                "  回答中..."
            };

            let label = format!("  🛸 Pilot {}", status);
            lines.push(Line::from(vec![
                Span::styled(
                    "  🛸 Pilot".to_string(),
                    Style::default().fg(Color::Rgb(100, 149, 237)).bold(),
                ),
                Span::styled(status.to_string(), Style::default().fg(Color::DarkGray)),
            ]));
            plain.push(label);
            // 流式推理过程
            if !self.streaming_reasoning.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    ┌─ 💭 思考 ──────────────────".to_string(),
                    Style::default().fg(Color::Rgb(100, 100, 120)),
                )));
                plain.push("    ┌─ 💭 思考 ──────────────────".to_string());
                let r_style = Style::default().fg(Color::Rgb(120, 120, 150));
                for rline in self.streaming_reasoning.lines() {
                    let prefixed = format!("    │ {}", rline);
                    wrap_line_into(&prefixed, w, r_style, &mut lines, &mut plain);
                }
                if !self.streaming_text.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    └───────────────────────────".to_string(),
                        Style::default().fg(Color::Rgb(100, 100, 120)),
                    )));
                    plain.push("    └───────────────────────────".to_string());
                }
            }
            // 流式内容
            let content_style = Style::default().fg(Color::Rgb(220, 220, 220));
            for line in self.streaming_text.lines() {
                let prefixed = format!("    {}", line);
                wrap_line_into(&prefixed, w, content_style, &mut lines, &mut plain);
            }
            // 闪烁光标效果
            lines.push(Line::from(Span::styled(
                "    █".to_string(),
                Style::default().fg(Color::Rgb(100, 149, 237)),
            )));
            plain.push("    █".to_string());
        }

        self.cached_lines = lines;
        self.plain_lines = plain;
        self.lines_dirty = false;
        self.last_width = width;
    }

    /// 通知面板视口大小发生变化，需要重建折行缓存
    pub fn on_resize(&mut self) {
        self.lines_dirty = true;
    }

    /// 渲染聊天面板
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" 💬 对话 ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 149, 237)))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // 缓存视口尺寸
        self.visible_height = inner.height;

        // 检测是否需要重建折行缓存（宽度变化或内容变化）
        if self.lines_dirty || self.last_width != inner.width {
            let was_at_bottom = self.auto_follow;
            self.rebuild_lines(inner.width);
            if was_at_bottom {
                self.scroll_position = self.max_scroll();
            } else {
                let max = self.max_scroll();
                if self.scroll_position > max {
                    self.scroll_position = max;
                }
            }
        }

        let total_lines = self.cached_lines.len();
        let visible = self.visible_height as usize;

        if total_lines == 0 {
            return;
        }

        let start = self.scroll_position.min(total_lines.saturating_sub(1));
        let end = (start + visible).min(total_lines);

        // 如果有选择，给选中行添加高亮背景
        let sel_range = self.selection.ordered().map(|(s, e)| (s.line, e.line));
        let visible_lines: Vec<Line> = (start..end)
            .map(|i| {
                let line = &self.cached_lines[i];
                if let Some((sel_start, sel_end)) = sel_range {
                    if i >= sel_start && i <= sel_end {
                        // 选中行：反转前景/背景色
                        let highlighted: Vec<Span> = line
                            .spans
                            .iter()
                            .map(|span| {
                                let bg = span.style.fg.unwrap_or(Color::Rgb(220, 220, 220));
                                Span::styled(
                                    span.content.clone(),
                                    Style::default()
                                        .fg(bg)
                                        .bg(Color::Rgb(60, 80, 120)),
                                )
                            })
                            .collect();
                        Line::from(highlighted)
                    } else {
                        line.clone()
                    }
                } else {
                    line.clone()
                }
            })
            .collect();

        let paragraph = Paragraph::new(visible_lines);
        frame.render_widget(paragraph, inner);

        // 滚动条
        let max_scroll = self.max_scroll();
        if max_scroll > 0 {
            let mut scrollbar_state =
                ScrollbarState::new(max_scroll).position(self.scroll_position);
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

/// 将一行文本按视口宽度预折行，结果追加到 `out`（styled lines）和 `plain_out`（纯文本）
fn wrap_line_into(text: &str, max_width: usize, style: Style, out: &mut Vec<Line<'static>>, plain_out: &mut Vec<String>) {
    if max_width == 0 {
        out.push(Line::from(Span::styled(text.to_string(), style)));
        plain_out.push(text.to_string());
        return;
    }

    let text_width = UnicodeWidthStr::width(text);
    if text_width <= max_width {
        out.push(Line::from(Span::styled(text.to_string(), style)));
        plain_out.push(text.to_string());
        return;
    }

    let mut current_line = String::new();
    let mut current_width: usize = 0;

    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);

        if current_width + ch_width > max_width && !current_line.is_empty() {
            out.push(Line::from(Span::styled(current_line.clone(), style)));
            plain_out.push(current_line.clone());
            current_line.clear();
            current_width = 0;
        }

        current_line.push(ch);
        current_width += ch_width;
    }

    if !current_line.is_empty() {
        out.push(Line::from(Span::styled(current_line.clone(), style)));
        plain_out.push(current_line);
    }
}
