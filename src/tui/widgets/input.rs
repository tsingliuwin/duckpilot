use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

/// 输入框组件
#[derive(Debug)]
pub struct InputBox {
    /// 当前输入内容
    pub content: String,
    /// 光标位置（字符索引）
    pub cursor_position: usize,
    /// 命令历史
    pub history: Vec<String>,
    /// 历史浏览索引
    pub history_index: Option<usize>,
    /// 是否获得焦点
    pub focused: bool,
    /// 临时保存的当前输入（浏览历史时）
    temp_content: String,
}

impl Default for InputBox {
    fn default() -> Self {
        Self {
            content: String::new(),
            cursor_position: 0,
            history: Vec::new(),
            history_index: None,
            focused: true,
            temp_content: String::new(),
        }
    }
}

impl InputBox {
    /// 处理键盘输入，返回是否有提交的内容
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<String> {
        match (key.code, key.modifiers) {
            // Enter 提交
            (KeyCode::Enter, KeyModifiers::NONE) => {
                if self.content.trim().is_empty() {
                    return None;
                }
                let submitted = self.content.clone();
                self.history.push(submitted.clone());
                self.content.clear();
                self.cursor_position = 0;
                self.history_index = None;
                Some(submitted)
            }

            // 退格删除
            (KeyCode::Backspace, _) => {
                if self.cursor_position > 0 {
                    let byte_pos = self.char_to_byte(self.cursor_position - 1);
                    let next_byte_pos = self.char_to_byte(self.cursor_position);
                    self.content.replace_range(byte_pos..next_byte_pos, "");
                    self.cursor_position -= 1;
                }
                None
            }

            // Delete 键
            (KeyCode::Delete, _) => {
                let char_count = self.content.chars().count();
                if self.cursor_position < char_count {
                    let byte_pos = self.char_to_byte(self.cursor_position);
                    let next_byte_pos = self.char_to_byte(self.cursor_position + 1);
                    self.content.replace_range(byte_pos..next_byte_pos, "");
                }
                None
            }

            // 左移光标
            (KeyCode::Left, _) => {
                self.cursor_position = self.cursor_position.saturating_sub(1);
                None
            }

            // 右移光标
            (KeyCode::Right, _) => {
                let char_count = self.content.chars().count();
                if self.cursor_position < char_count {
                    self.cursor_position += 1;
                }
                None
            }

            // Home
            (KeyCode::Home, _) => {
                self.cursor_position = 0;
                None
            }

            // End
            (KeyCode::End, _) => {
                self.cursor_position = self.content.chars().count();
                None
            }

            // 上翻历史
            (KeyCode::Up, _) => {
                if !self.history.is_empty() {
                    match self.history_index {
                        None => {
                            self.temp_content = self.content.clone();
                            self.history_index = Some(self.history.len() - 1);
                        }
                        Some(idx) if idx > 0 => {
                            self.history_index = Some(idx - 1);
                        }
                        _ => {}
                    }
                    if let Some(idx) = self.history_index {
                        self.content = self.history[idx].clone();
                        self.cursor_position = self.content.chars().count();
                    }
                }
                None
            }

            // 下翻历史
            (KeyCode::Down, _) => {
                if let Some(idx) = self.history_index {
                    if idx < self.history.len() - 1 {
                        self.history_index = Some(idx + 1);
                        self.content = self.history[idx + 1].clone();
                    } else {
                        self.history_index = None;
                        self.content = self.temp_content.clone();
                    }
                    self.cursor_position = self.content.chars().count();
                }
                None
            }

            // Ctrl+U 清空行
            (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                self.content.clear();
                self.cursor_position = 0;
                None
            }

            // 普通字符输入
            (KeyCode::Char(c), _) => {
                let byte_pos = self.char_to_byte(self.cursor_position);
                self.content.insert(byte_pos, c);
                self.cursor_position += 1;
                None
            }

            _ => None,
        }
    }

    /// 将字符索引转换为字节索引
    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.content
            .char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(self.content.len())
    }

    /// 渲染输入框
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focused {
            Color::Rgb(100, 149, 237) // 焦点态：蓝色
        } else {
            Color::Rgb(80, 80, 80) // 非焦点：暗灰
        };

        let block = Block::default()
            .title(" ✏️  输入 (Enter 发送 | Esc 退出) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let display_text = if self.content.is_empty() && !self.focused {
            "输入自然语言问题..."
        } else if self.content.is_empty() {
            ""
        } else {
            &self.content
        };

        let style = if self.content.is_empty() && !self.focused {
            Style::default().fg(Color::DarkGray).italic()
        } else {
            Style::default().fg(Color::Rgb(240, 240, 240))
        };

        let paragraph = Paragraph::new(display_text)
            .style(style)
            .block(block);

        frame.render_widget(paragraph, area);

        // 渲染光标
        if self.focused {
            let inner = Rect {
                x: area.x + 1,
                y: area.y + 1,
                width: area.width.saturating_sub(2),
                height: area.height.saturating_sub(2),
            };

            // 计算光标显示宽度（考虑中文字符）
            let cursor_display_width: usize = self
                .content
                .chars()
                .take(self.cursor_position)
                .map(|c| if c.is_ascii() { 1 } else { 2 })
                .sum();

            let cursor_x = inner.x + cursor_display_width as u16;
            let cursor_y = inner.y;

            if cursor_x < inner.x + inner.width {
                frame.set_cursor_position((cursor_x, cursor_y));
            }
        }
    }
}
