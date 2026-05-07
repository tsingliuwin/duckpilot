use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
};
use crate::tui::event::TableSchema;

/// Schema 侧边栏
#[derive(Debug)]
pub struct SchemaPanel {
    pub schemas: Vec<TableSchema>,
    pub list_state: ListState,
    pub expanded: Vec<bool>,
    pub focused: bool,
}

impl Default for SchemaPanel {
    fn default() -> Self {
        Self { schemas: Vec::new(), list_state: ListState::default(), expanded: Vec::new(), focused: false }
    }
}

impl SchemaPanel {
    pub fn set_schemas(&mut self, schemas: Vec<TableSchema>) {
        self.expanded = vec![false; schemas.len()];
        self.schemas = schemas;
        if !self.schemas.is_empty() { self.list_state.select(Some(0)); }
    }

    pub fn toggle_expand(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            let mut current = 0;
            for (table_idx, schema) in self.schemas.iter().enumerate() {
                if current == idx { self.expanded[table_idx] = !self.expanded[table_idx]; break; }
                current += 1;
                if self.expanded[table_idx] { current += schema.columns.len(); }
            }
        }
    }

    pub fn next(&mut self) {
        let total = self.total_items();
        if total == 0 { return; }
        let i = self.list_state.selected().map(|i| (i + 1) % total).unwrap_or(0);
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let total = self.total_items();
        if total == 0 { return; }
        let i = self.list_state.selected().map(|i| if i == 0 { total - 1 } else { i - 1 }).unwrap_or(0);
        self.list_state.select(Some(i));
    }

    fn total_items(&self) -> usize {
        self.schemas.iter().enumerate().map(|(idx, s)| 1 + if self.expanded[idx] { s.columns.len() } else { 0 }).sum()
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focused { Color::Rgb(100, 149, 237) } else { Color::Rgb(80, 80, 80) };
        let block = Block::default().title(" 📊 数据结构 ").borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)).border_type(ratatui::widgets::BorderType::Rounded);

        if self.schemas.is_empty() {
            let hint = ratatui::widgets::Paragraph::new("  暂无数据\n\n  请先运行:\n  duckpilot init")
                .style(Style::default().fg(Color::DarkGray)).block(block);
            frame.render_widget(hint, area);
            return;
        }

        let mut items: Vec<ListItem> = Vec::new();
        for (table_idx, schema) in self.schemas.iter().enumerate() {
            let icon = if self.expanded[table_idx] { "▼" } else { "▶" };
            let row_info = schema.row_count.map(|c| format!(" ({}行)", c)).unwrap_or_default();
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", icon), Style::default().fg(Color::Rgb(255, 183, 77))),
                Span::styled(format!("📄 {}", schema.name), Style::default().fg(Color::Rgb(129, 199, 132)).bold()),
                Span::styled(row_info, Style::default().fg(Color::DarkGray)),
            ])));
            if self.expanded[table_idx] {
                for col in &schema.columns {
                    let tc = match col.data_type.to_uppercase().as_str() {
                        s if s.contains("INT") || s.contains("FLOAT") || s.contains("DOUBLE") => Color::Rgb(144, 202, 249),
                        s if s.contains("VARCHAR") || s.contains("TEXT") => Color::Rgb(206, 147, 216),
                        s if s.contains("DATE") || s.contains("TIME") => Color::Rgb(255, 183, 77),
                        s if s.contains("BOOL") => Color::Rgb(129, 199, 132),
                        _ => Color::Rgb(180, 180, 180),
                    };
                    items.push(ListItem::new(Line::from(vec![
                        Span::raw("     "), Span::styled(&col.name, Style::default().fg(Color::Rgb(220, 220, 220))),
                        Span::styled(format!(" : {}", col.data_type), Style::default().fg(tc)),
                    ])));
                }
            }
        }

        let list = List::new(items).block(block)
            .highlight_style(Style::default().bg(Color::Rgb(40, 44, 52)).add_modifier(Modifier::BOLD))
            .highlight_symbol("│");
        frame.render_stateful_widget(list, area, &mut self.list_state);
    }
}
