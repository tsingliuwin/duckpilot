use ratatui::{prelude::*, widgets::{Block, Borders, Row, Table, Cell, TableState}};
use crate::tui::event::QueryResultData;

/// 表格视图组件
#[derive(Debug)]
pub struct TableView {
    pub data: Option<QueryResultData>,
    state: TableState,
}

impl Default for TableView {
    fn default() -> Self {
        let mut state = TableState::default();
        state.select(Some(0));
        Self { data: None, state }
    }
}

impl TableView {
    pub fn set_data(&mut self, data: QueryResultData) {
        self.data = Some(data);
        self.state.select(Some(0));
    }

    pub fn scroll_up(&mut self) {
        if let Some(i) = self.state.selected() {
            if i > 0 {
                self.state.select(Some(i - 1));
            }
        }
    }

    pub fn scroll_down(&mut self) {
        if let Some(data) = &self.data {
            if let Some(i) = self.state.selected() {
                if i < data.rows.len().saturating_sub(1) {
                    self.state.select(Some(i + 1));
                }
            }
        }
    }

    /// 按指定行数滚动（支持鼠标滚轮连续滚动）
    pub fn scroll_by(&mut self, delta: i32) {
        if let Some(data) = &self.data {
            let current = self.state.selected().unwrap_or(0);
            let max = data.rows.len().saturating_sub(1);
            let new_pos = (current as i64 + delta as i64)
                .max(0)
                .min(max as i64) as usize;
            self.state.select(Some(new_pos));
        }
    }

    pub fn page_up(&mut self, visible_rows: usize) {
        let step = visible_rows.max(1);
        for _ in 0..step {
            self.scroll_up();
        }
    }

    pub fn page_down(&mut self, visible_rows: usize) {
        let step = visible_rows.max(1);
        for _ in 0..step {
            self.scroll_down();
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default().title(" 📋 查询结果 ").borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 149, 237)))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let Some(data) = &self.data else {
            let hint = ratatui::widgets::Paragraph::new("  输入问题后，查询结果将显示在此处")
                .style(Style::default().fg(Color::DarkGray)).block(block);
            frame.render_widget(hint, area);
            return;
        };

        if data.rows.is_empty() {
            let hint = ratatui::widgets::Paragraph::new("  查询结果为空")
                .style(Style::default().fg(Color::DarkGray)).block(block);
            frame.render_widget(hint, area);
            return;
        }

        let header_cells: Vec<Cell> = data.columns.iter()
            .map(|h| Cell::from(h.as_str()).style(Style::default().fg(Color::Rgb(255, 183, 77)).bold()))
            .collect();
        let header = Row::new(header_cells).height(1).bottom_margin(1)
            .style(Style::default().bg(Color::Rgb(35, 35, 45)));

        let rows: Vec<Row> = data.rows.iter().enumerate().map(|(i, row)| {
            let bg = if i == self.state.selected().unwrap_or(0) { Color::Rgb(40, 44, 52) }
                else if i % 2 == 0 { Color::Rgb(28, 28, 35) }
                else { Color::Rgb(22, 22, 28) };
            let cells: Vec<Cell> = row.iter().map(|c| Cell::from(c.as_str())).collect();
            Row::new(cells).style(Style::default().bg(bg).fg(Color::Rgb(200, 200, 200)))
        }).collect();

        let widths: Vec<Constraint> = data.columns.iter().map(|_| Constraint::Min(8)).collect();

        let footer = format!(" {} 行 × {} 列 | 耗时 {}ms ", data.row_count, data.columns.len(), data.execution_time_ms);
        let table = Table::new(rows, &widths).header(header).block(
            block.title_bottom(Line::from(footer).right_aligned())
        ).row_highlight_style(
            Style::default().bg(Color::Rgb(50, 60, 90)).add_modifier(Modifier::BOLD)
        );

        frame.render_stateful_widget(table, area, &mut self.state);
    }
}
