use anyhow::Result;
use crossterm::{
    execute,
    event::{EnableMouseCapture, DisableMouseCapture},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::{self, stdout};

/// Terminal 管理器，负责初始化和恢复终端状态
pub struct TerminalManager;

impl TerminalManager {
    /// 初始化终端：启用 raw mode，进入备用屏幕，启用鼠标捕获
    pub fn init() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
        // 安装 panic hook，确保异常退出时恢复终端
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = Self::restore();
            original_hook(panic_info);
        }));

        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;

        Ok(terminal)
    }

    /// 恢复终端到正常状态
    pub fn restore() -> Result<()> {
        disable_raw_mode()?;
        execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
        Ok(())
    }
}
