use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, FocusArea, ViewMode};
/// 主界面渲染
pub fn render(app: &mut App, frame: &mut Frame) {
    let size = frame.area();

    // 全局背景色
    let bg = Block::default().style(Style::default().bg(Color::Rgb(18, 18, 24)));
    frame.render_widget(bg, size);

    // 整体垂直布局：标题栏 | 主内容 | 输入框 | 状态栏
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // 标题栏
            Constraint::Min(10),   // 主内容区
            Constraint::Length(3), // 输入框
            Constraint::Length(2), // 状态栏
        ])
        .split(size);

    // 渲染标题栏
    render_title_bar(app, frame, main_layout[0]);

    // 主内容区：左侧 Schema + 右侧（聊天/结果）
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25), // Schema 面板
            Constraint::Percentage(75), // 主面板
        ])
        .split(main_layout[1]);

    // Schema 面板
    app.schema_panel.focused = matches!(app.focus, FocusArea::Schema);
    app.schema_panel.render(frame, content_layout[0]);
    app.schema_panel_area = Some(content_layout[0]);

    // 右侧面板（根据模式切换）
    match app.view_mode {
        ViewMode::Chat => {
            app.chat_panel.render(frame, content_layout[1]);
            app.chat_panel_area = Some(content_layout[1]);
            app.table_view_area = None;
        }
        ViewMode::Table => {
            app.table_view.render(frame, content_layout[1]);
            app.chat_panel_area = None;
            app.table_view_area = Some(content_layout[1]);
        }
        ViewMode::Split => {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(content_layout[1]);
            app.chat_panel.render(frame, split[0]);
            app.table_view.render(frame, split[1]);
            app.chat_panel_area = Some(split[0]);
            app.table_view_area = Some(split[1]);
        }
    }

    // 输入框
    app.input_box.focused = matches!(app.focus, FocusArea::Input);
    app.input_box.render(frame, main_layout[2]);
    app.input_box_area = Some(main_layout[2]);

    // 状态栏
    app.status_bar.render(frame, main_layout[3]);
}

/// 渲染标题栏
fn render_title_bar(app: &App, frame: &mut Frame, area: Rect) {
    let title = vec![
        Span::styled("  🛸 ", Style::default().fg(Color::Rgb(255, 183, 77))),
        Span::styled(
            "DuckPilot",
            Style::default()
                .fg(Color::Rgb(100, 149, 237))
                .bold(),
        ),
        Span::styled(
            format!("  v{}", env!("CARGO_PKG_VERSION")),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 60))),
        Span::styled(
            format!("📁 {}", app.status_bar.project_name),
            Style::default().fg(Color::Rgb(179, 157, 219)),
        ),
        Span::styled("  │  ", Style::default().fg(Color::Rgb(60, 60, 60))),
        Span::styled(
            format!("📡 {}", app.status_bar.model_name),
            Style::default().fg(Color::Rgb(129, 199, 132)),
        ),
    ];

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 60)))
        .style(Style::default().bg(Color::Rgb(25, 25, 35)));

    let paragraph = Paragraph::new(Line::from(title))
        .block(block)
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}
