use ratatui::{prelude::*, widgets::{Block, Borders, Row, Table, Cell}};
use crate::tui::event::QueryResultData;

/// 表格视图组件
#[derive(Debug)]
pub struct TableView {
    pub data: Option<QueryResultData>,
    pub scroll_offset: usize,
    pub selected_row: usize,
}

impl Default for TableView {
    fn default() -> Self {
        Self { data: None, scroll_offset: 0, selected_row: 0 }
    }
}

impl TableView {
    pub fn set_data(&mut self, data: QueryResultData) {
        self.data = Some(data);
        self.scroll_offset = 0;
        self.selected_row = 0;
    }

    pub fn clear(&mut self) { self.data = None; }

    pub fn scroll_up(&mut self) { self.selected_row = self.selected_row.saturating_sub(1); }

    pub fn scroll_down(&mut self) {
        if let Some(data) = &self.data {
            if self.selected_row < data.rows.len().saturating_sub(1) { self.selected_row += 1; }
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default().title(" 📋 查询结果 ").borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Rgb(100, 149, 237)))
            .border_type(ratatui::widgets::BorderType::Rounded);

        let Some(data) = &self.data else {
            let hint = ratatui::widgets::Paragraph::new("  输入问题后，查询结果将显示在此处")
                .style(Style::default().fg(Color::DarkGray)).block(block);
            frame.render_widget(hint, area);
            return;
        };

        let header_cells: Vec<Cell> = data.columns.iter()
            .map(|h| Cell::from(h.as_str()).style(Style::default().fg(Color::Rgb(255, 183, 77)).bold()))
            .collect();
        let header = Row::new(header_cells).height(1).bottom_margin(1)
            .style(Style::default().bg(Color::Rgb(35, 35, 45)));

        let rows: Vec<Row> = data.rows.iter().enumerate().map(|(i, row)| {
            let bg = if i == self.selected_row { Color::Rgb(40, 44, 52) }
                else if i % 2 == 0 { Color::Rgb(28, 28, 35) }
                else { Color::Rgb(22, 22, 28) };
            let cells: Vec<Cell> = row.iter().map(|c| Cell::from(c.as_str())).collect();
            Row::new(cells).style(Style::default().bg(bg).fg(Color::Rgb(200, 200, 200)))
        }).collect();

        let widths: Vec<Constraint> = data.columns.iter().map(|_| Constraint::Min(8)).collect();

        let footer = format!(" {} 行 × {} 列 | 耗时 {}ms ", data.row_count, data.columns.len(), data.execution_time_ms);
        let table = Table::new(rows, &widths).header(header).block(
            block.title_bottom(Line::from(footer).right_aligned())
        );
        frame.render_widget(table, area);
    }
}
