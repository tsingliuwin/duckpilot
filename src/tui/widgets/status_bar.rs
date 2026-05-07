use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

/// 状态栏数据
#[derive(Debug, Clone)]
pub struct StatusBarData {
    pub db_connected: bool,
    pub llm_configured: bool,
    pub model_name: String,
    pub project_name: String,
    pub data_files_count: usize,
}

impl Default for StatusBarData {
    fn default() -> Self {
        Self {
            db_connected: false,
            llm_configured: false,
            model_name: "gpt-4o".to_string(),
            project_name: "未初始化".to_string(),
            data_files_count: 0,
        }
    }
}

impl StatusBarData {
    /// 渲染状态栏
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let db_status = if self.db_connected {
            Span::styled(" DB:✓ ", Style::default().fg(Color::Rgb(129, 199, 132)))
        } else {
            Span::styled(" DB:✗ ", Style::default().fg(Color::Rgb(239, 83, 80)))
        };

        let llm_status = if self.llm_configured {
            Span::styled(" LLM:✓ ", Style::default().fg(Color::Rgb(129, 199, 132)))
        } else {
            Span::styled(" LLM:✗ ", Style::default().fg(Color::Rgb(255, 183, 77)))
        };

        let model = Span::styled(
            format!(" 📡 {} ", self.model_name),
            Style::default().fg(Color::Rgb(179, 157, 219)),
        );

        let files = Span::styled(
            format!(" 📁 {}个数据文件 ", self.data_files_count),
            Style::default().fg(Color::Rgb(144, 202, 249)),
        );

        let separator = Span::styled(" │ ", Style::default().fg(Color::Rgb(80, 80, 80)));

        let shortcuts = Span::styled(
            " Esc:退出 │ Tab:切换面板 │ Ctrl+R:刷新 Schema ",
            Style::default().fg(Color::DarkGray),
        );

        let line = Line::from(vec![
            db_status,
            separator.clone(),
            llm_status,
            separator.clone(),
            model,
            separator.clone(),
            files,
            separator,
            shortcuts,
        ]);

        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 60)));

        let paragraph = Paragraph::new(line)
            .block(block)
            .style(Style::default().bg(Color::Rgb(25, 25, 30)));

        frame.render_widget(paragraph, area);
    }
}
