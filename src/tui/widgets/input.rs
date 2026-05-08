use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders},
};
use tui_textarea::{CursorMove, TextArea};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 输入框组件（基于 tui-textarea 实现）
pub struct InputBox {
    textarea: TextArea<'static>,
    history: Vec<String>,
    history_index: Option<usize>,
    temp_content: String,
    /// 是否获得焦点
    pub focused: bool,
}

impl Default for InputBox {
    fn default() -> Self {
        let textarea = TextArea::default();
        Self {
            textarea,
            history: Vec::new(),
            history_index: None,
            temp_content: String::new(),
            focused: true,
        }
    }
}

impl InputBox {
    /// 处理键盘输入，返回是否有提交的内容
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        match (key.code, key.modifiers) {
            // Enter 提交
            (KeyCode::Enter, KeyModifiers::NONE) => {
                let text = self.textarea.lines().join("\n");
                if text.trim().is_empty() {
                    return None;
                }
                self.history.push(text.clone());
                self.clear_text();
                self.history_index = None;
                Some(text)
            }

            // Shift+Enter 或 Alt+Enter 换行
            (KeyCode::Enter, KeyModifiers::SHIFT) | (KeyCode::Enter, KeyModifiers::ALT) => {
                self.textarea.insert_newline();
                None
            }

            // 上翻历史
            (KeyCode::Up, KeyModifiers::NONE) => {
                if !self.history.is_empty() {
                    match self.history_index {
                        None => {
                            self.temp_content = self.textarea.lines().join("\n");
                            self.history_index = Some(self.history.len() - 1);
                        }
                        Some(idx) if idx > 0 => {
                            self.history_index = Some(idx - 1);
                        }
                        _ => {}
                    }
                    if let Some(idx) = self.history_index {
                        let text = self.history[idx].clone();
                        self.set_text(&text);
                    }
                }
                None
            }

            // 下翻历史
            (KeyCode::Down, KeyModifiers::NONE) => {
                if let Some(idx) = self.history_index {
                    if idx < self.history.len() - 1 {
                        self.history_index = Some(idx + 1);
                        let text = self.history[idx + 1].clone();
                        self.set_text(&text);
                    } else {
                        self.history_index = None;
                        let text = self.temp_content.clone();
                        self.set_text(&text);
                    }
                }
                None
            }

            // Ctrl+A 全选
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                self.textarea.select_all();
                None
            }

            // Ctrl+C 复制选中文本
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                let yanked = self.textarea.yank_text();
                if !yanked.is_empty() {
                    let _ = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(&yanked));
                }
                None
            }

            // Ctrl+V 粘贴文本
            (KeyCode::Char('v'), KeyModifiers::CONTROL) => {
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    if let Ok(text) = clipboard.get_text() {
                        let text = text.replace("\r\n", "\n");
                        self.textarea.insert_str(text);
                    }
                }
                None
            }

            // 其他按键交给 textarea 处理
            _ => {
                self.textarea.input(key);
                None
            }
        }
    }

    /// 处理鼠标事件（点击定位光标 + 拖拽选择文本）
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) {
        let inner_y = mouse.row.saturating_sub(area.y + 1) as usize;
        let inner_x = mouse.column.saturating_sub(area.x + 1) as usize;

        let inner_h = area.height.saturating_sub(1) as usize;
        let (cur_row, _) = self.textarea.cursor();
        let top_row = if cur_row >= inner_h {
            cur_row + 1 - inner_h
        } else {
            0
        };

        let text_row = (inner_y + top_row).min(self.textarea.lines().len().saturating_sub(1));
        let line = &self.textarea.lines()[text_row];
        let text_col = visual_col_to_char_index(line, inner_x);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.textarea
                    .move_cursor(CursorMove::Jump(text_row as u16, text_col as u16));
                self.textarea.start_selection();
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.textarea
                    .move_cursor(CursorMove::Jump(text_row as u16, text_col as u16));
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let yanked = self.textarea.yank_text();
                if !yanked.is_empty() {
                    let _ = arboard::Clipboard::new().and_then(|mut cb| cb.set_text(&yanked));
                }
            }
            _ => {}
        }
    }

    fn clear_text(&mut self) {
        self.textarea = TextArea::default();
    }

    fn set_text(&mut self, text: &str) {
        let lines: Vec<String> = text.split('\n').map(String::from).collect();
        self.textarea = TextArea::new(lines);
        self.textarea.move_cursor(tui_textarea::CursorMove::Bottom);
        self.textarea.move_cursor(tui_textarea::CursorMove::End);
    }

    /// 渲染输入框
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focused {
            Color::Rgb(100, 149, 237)
        } else {
            Color::Rgb(80, 80, 80)
        };

        self.textarea.set_block(
            Block::default()
                .title(" ✏️  输入 (Enter 发送 | Shift+Enter 换行 | Esc 退出) ")
                .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                .border_style(Style::default().fg(border_color))
                .border_type(ratatui::widgets::BorderType::Rounded),
        );

        let style = if self.textarea.lines().join("").is_empty() && !self.focused {
            Style::default().fg(Color::DarkGray).italic()
        } else {
            Style::default().fg(Color::Rgb(240, 240, 240))
        };
        self.textarea.set_style(style);

        // 不使用 REVERSED 软件光标，完全依赖终端硬件光标
        self.textarea.set_cursor_style(Style::default());
        // 禁用光标行下划线高亮
        self.textarea.set_cursor_line_style(Style::default());

        frame.render_widget(&self.textarea, area);

        // 设置终端光标位置，使 IME 候选窗口正确定位
        if self.focused {
            let (row, col) = self.textarea.cursor();
            let inner_h = area.height.saturating_sub(2) as usize;
            let top_row = if row >= inner_h { row + 1 - inner_h } else { 0 };
            let y = area.y + 1 + (row - top_row) as u16;
            // col 是字符索引，需要转换为视觉列宽（中文占2列）
            let prefix: String = self.textarea.lines()[row].chars().take(col).collect();
            let visual_col = UnicodeWidthStr::width(prefix.as_str()) as u16;
            let x = area.x + 1 + visual_col;
            frame.set_cursor_position((x, y));
        }
    }
}

/// 将视觉列位置转换为字符索引（中文字符占2列）
fn visual_col_to_char_index(line: &str, visual_col: usize) -> usize {
    let mut width = 0;
    for (i, c) in line.chars().enumerate() {
        if width >= visual_col {
            return i;
        }
        width += UnicodeWidthChar::width(c).unwrap_or(1);
    }
    line.chars().count()
}
