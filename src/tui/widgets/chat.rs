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
    /// 内容是否发生了变化，需要重建缓存
    lines_dirty: bool,

    /// 鼠标选择范围：(start_line, start_col) 到 (end_line, end_col)
    selection: Option<((usize, usize), (usize, usize))>,
    /// 是否正在通过鼠标拖拽进行选择
    is_selecting: bool,
    /// 上次渲染时的内容区域（不含边框），用于鼠标坐标换算
    last_inner_area: Rect,

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
            scroll_position: 0,
            auto_follow: true,
            visible_height: 0,
            last_width: 0,
            cached_lines: Vec::new(),
            lines_dirty: true,
            selection: None,
            is_selecting: false,
            last_inner_area: Rect::default(),
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

    /// 清除选择
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// 处理鼠标事件以支持选择
    pub fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        use crossterm::event::MouseEventKind;
        let area = self.last_inner_area;
        
        // 滚轮事件
        if matches!(mouse.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
            match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_by(-3),
                MouseEventKind::ScrollDown => self.scroll_by(3),
                _ => {}
            }
            return;
        }

        // 只有在面板区域内的点击才触发选择
        if mouse.column < area.x || mouse.column >= area.x + area.width ||
           mouse.row < area.y || mouse.row >= area.y + area.height {
            if matches!(mouse.kind, MouseEventKind::Down(_)) {
                self.clear_selection();
            }
            return;
        }

        let rel_x = (mouse.column - area.x) as usize;
        let rel_y = (mouse.row - area.y) as usize;
        let line_idx = self.scroll_position + rel_y;

        if line_idx >= self.cached_lines.len() {
            return;
        }

        // 计算字符索引
        let char_idx = self.get_char_index_at(line_idx, rel_x);

        match mouse.kind {
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                self.selection = Some(((line_idx, char_idx), (line_idx, char_idx)));
                self.is_selecting = true;
            }
            MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                if let Some((start, _)) = self.selection {
                    self.selection = Some((start, (line_idx, char_idx)));
                }
            }
            MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                self.is_selecting = false;
                // 如果选择范围太小，视为点击清除
                if let Some((s, e)) = self.selection {
                    if s == e {
                        self.clear_selection();
                    }
                }
            }
            _ => {}
        }
    }

    fn get_char_index_at(&self, line_idx: usize, x: usize) -> usize {
        if line_idx >= self.cached_lines.len() { return 0; }
        let line = &self.cached_lines[line_idx];
        let mut current_width = 0;
        let mut char_count = 0;
        for span in &line.spans {
            for ch in span.content.chars() {
                let w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
                if current_width + w > x {
                    return char_count;
                }
                current_width += w;
                char_count += 1;
            }
        }
        char_count
    }

    /// 获取选中的文本
    pub fn get_selected_text(&self) -> Option<String> {
        let ((s_line, s_char), (e_line, e_char)) = self.selection?;
        let (start, end) = if (s_line, s_char) <= (e_line, e_char) {
            ((s_line, s_char), (e_line, e_char))
        } else {
            ((e_line, e_char), (s_line, s_char))
        };

        let mut out = String::new();
        for i in start.0..=end.0 {
            if i >= self.cached_lines.len() { break; }
            let line_text: String = self.cached_lines[i].spans.iter().map(|s| s.content.as_ref()).collect();
            let chars: Vec<char> = line_text.chars().collect();
            
            let line_start = if i == start.0 { start.1 } else { 0 };
            let line_end = if i == end.0 { end.1.min(chars.len()) } else { chars.len() };
            
            if line_start < chars.len() {
                out.push_str(&chars[line_start..line_end].iter().collect::<String>());
            }
            if i < end.0 {
                out.push('\n');
            }
        }
        if out.is_empty() { None } else { Some(out) }
    }

    /// 当前最大可滚动行数
    fn max_scroll(&self) -> usize {
        self.cached_lines.len().saturating_sub(self.visible_height as usize)
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
                Span::styled(prefix.to_string(), style),
                Span::styled(
                    format!("  {}", msg.timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));

            // 消息内容（预折行）
            let content_style = Style::default().fg(Color::Rgb(220, 220, 220));
            for line in msg.content.lines() {
                let prefixed = format!("    {}", line);
                wrap_line_into(&prefixed, w, content_style, &mut lines);
            }

            // 推理过程
            if let Some(reasoning) = &msg.reasoning {
                if msg.show_reasoning && !reasoning.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    ┌─ 💭 思考 ──────────────────".to_string(),
                        Style::default().fg(Color::Rgb(100, 100, 120)),
                    )));
                    let r_style = Style::default().fg(Color::Rgb(120, 120, 150));
                    for rline in reasoning.lines() {
                        let prefixed = format!("    │ {}", rline);
                        wrap_line_into(&prefixed, w, r_style, &mut lines);
                    }
                    lines.push(Line::from(Span::styled(
                        "    └───────────────────────────".to_string(),
                        Style::default().fg(Color::Rgb(100, 100, 120)),
                    )));
                } else if !reasoning.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    💭 思考过程已隐藏".to_string(),
                        Style::default().fg(Color::Rgb(80, 80, 100)),
                    )));
                }
            }

            // SQL 代码块
            if let Some(sql) = &msg.sql {
                lines.push(Line::from(Span::styled(
                    "    ┌─ SQL ──────────────────".to_string(),
                    Style::default().fg(Color::Rgb(150, 150, 150)),
                )));
                let sql_style = Style::default().fg(Color::Rgb(206, 147, 216));
                for sql_line in sql.lines() {
                    let prefixed = format!("    │ {}", sql_line);
                    wrap_line_into(&prefixed, w, sql_style, &mut lines);
                }
                lines.push(Line::from(Span::styled(
                    "    └───────────────────────".to_string(),
                    Style::default().fg(Color::Rgb(150, 150, 150)),
                )));
            }

            // 消息间空行
            lines.push(Line::from("".to_string()));
        }

        // 流式输出中的文本
        if self.is_streaming {
            let status = if self.streaming_text.is_empty() {
                "  思考中..."
            } else {
                "  回答中..."
            };

            lines.push(Line::from(vec![
                Span::styled(
                    "  🛸 Pilot".to_string(),
                    Style::default().fg(Color::Rgb(100, 149, 237)).bold(),
                ),
                Span::styled(status.to_string(), Style::default().fg(Color::DarkGray)),
            ]));
            // 流式推理过程
            if !self.streaming_reasoning.is_empty() {
                lines.push(Line::from(Span::styled(
                    "    ┌─ 💭 思考 ──────────────────".to_string(),
                    Style::default().fg(Color::Rgb(100, 100, 120)),
                )));
                let r_style = Style::default().fg(Color::Rgb(120, 120, 150));
                for rline in self.streaming_reasoning.lines() {
                    let prefixed = format!("    │ {}", rline);
                    wrap_line_into(&prefixed, w, r_style, &mut lines);
                }
                // 如果已经有正文了，推理过程可能还没显式结束，但我们可以给个视觉提示
                if !self.streaming_text.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "    └───────────────────────────".to_string(),
                        Style::default().fg(Color::Rgb(100, 100, 120)),
                    )));
                }
            }
            // 流式内容
            let content_style = Style::default().fg(Color::Rgb(220, 220, 220));
            for line in self.streaming_text.lines() {
                let prefixed = format!("    {}", line);
                wrap_line_into(&prefixed, w, content_style, &mut lines);
            }
            // 闪烁光标效果
            lines.push(Line::from(Span::styled(
                "    █".to_string(),
                Style::default().fg(Color::Rgb(100, 149, 237)),
            )));
        }

        self.cached_lines = lines;
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

        // 缓存视口尺寸和区域
        self.visible_height = inner.height;
        self.last_inner_area = inner;

        // 检测是否需要重建折行缓存（宽度变化或内容变化）
        if self.lines_dirty || self.last_width != inner.width {
            let was_at_bottom = self.auto_follow;
            self.rebuild_lines(inner.width);
            // 重建后，如果之前是跟随底部，保持在底部
            if was_at_bottom {
                self.scroll_position = self.max_scroll();
            } else {
                // 钳位 scroll_position，防止溢出
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

        // 直接取切片渲染，不使用 Paragraph::wrap 和 scroll
        let start = self.scroll_position.min(total_lines.saturating_sub(1));
        let end = (start + visible).min(total_lines);
        
        let mut visible_lines = Vec::new();
        for i in start..end {
            let line = &self.cached_lines[i];
            if let Some(sel) = self.selection {
                visible_lines.push(self.apply_selection_to_line(line, i, sel));
            } else {
                visible_lines.push(line.clone());
            }
        }

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

    fn apply_selection_to_line(&self, line: &Line<'static>, line_idx: usize, selection: ((usize, usize), (usize, usize))) -> Line<'static> {
        let (s, e) = if selection.0 <= selection.1 { (selection.0, selection.1) } else { (selection.1, selection.0) };
        
        // 如果行完全在选择范围外
        if line_idx < s.0 || line_idx > e.0 {
            return line.clone();
        }

        // 提取行文本并转为字符向量
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let chars: Vec<char> = line_text.chars().collect();
        
        // 计算本行的选择范围
        let sel_start = if line_idx == s.0 { s.1 } else { 0 };
        let sel_end = if line_idx == e.0 { e.1.min(chars.len()) } else { chars.len() };

        if sel_start >= sel_end {
            return line.clone();
        }

        // 重新构建 Spans
        let mut new_spans = Vec::new();
        let mut current_pos = 0;
        let selection_style = Style::default().bg(Color::Rgb(60, 100, 150)).fg(Color::White);

        for span in &line.spans {
            let span_chars: Vec<char> = span.content.chars().collect();
            let span_len = span_chars.len();
            let span_end_pos = current_pos + span_len;

            // 如果整个 span 都在选择范围内
            if current_pos >= sel_start && span_end_pos <= sel_end {
                new_spans.push(Span::styled(span.content.clone(), span.style.patch(selection_style)));
            }
            // 如果整个 span 都在选择范围外
            else if span_end_pos <= sel_start || current_pos >= sel_end {
                new_spans.push(span.clone());
            }
            // 部分重叠
            else {
                let overlap_start = sel_start.saturating_sub(current_pos).max(0);
                let overlap_end = (sel_end - current_pos).min(span_len);

                if overlap_start > 0 {
                    new_spans.push(Span::styled(span_chars[0..overlap_start].iter().collect::<String>(), span.style));
                }
                new_spans.push(Span::styled(span_chars[overlap_start..overlap_end].iter().collect::<String>(), span.style.patch(selection_style)));
                if overlap_end < span_len {
                    new_spans.push(Span::styled(span_chars[overlap_end..].iter().collect::<String>(), span.style));
                }
            }
            current_pos += span_len;
        }

        Line::from(new_spans)
    }
}

/// 将一行文本按视口宽度预折行，结果追加到 `out`
fn wrap_line_into(text: &str, max_width: usize, style: Style, out: &mut Vec<Line<'static>>) {
    if max_width == 0 {
        out.push(Line::from(Span::styled(text.to_string(), style)));
        return;
    }

    let text_width = UnicodeWidthStr::width(text);
    if text_width <= max_width {
        // 不需要折行
        out.push(Line::from(Span::styled(text.to_string(), style)));
        return;
    }

    // 需要折行：按字符逐个累加宽度
    let mut current_line = String::new();
    let mut current_width: usize = 0;

    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);

        if current_width + ch_width > max_width && !current_line.is_empty() {
            // 当前行满了，输出并换行
            out.push(Line::from(Span::styled(current_line.clone(), style)));
            current_line.clear();
            current_width = 0;
        }

        current_line.push(ch);
        current_width += ch_width;
    }

    // 最后一段
    if !current_line.is_empty() {
        out.push(Line::from(Span::styled(current_line, style)));
    }
}
