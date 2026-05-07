use std::time::{Duration, Instant};

/// 鼠标滚动方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
}

impl ScrollDirection {
    fn sign(self) -> i32 {
        match self {
            ScrollDirection::Up => -1,
            ScrollDirection::Down => 1,
        }
    }
}

/// 鼠标滚动状态跟踪
///
/// 区分触控板（高频事件，< 35ms 间隔，每次 1 行）和鼠标滚轮（低频，每次 3 行）。
/// 参考 DeepSeek-TUI 的 MouseScrollState 实现。
#[derive(Debug, Default)]
pub struct MouseScrollState {
    last_event_at: Option<Instant>,
}

impl MouseScrollState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 处理滚动事件，返回滚动行数（带符号）
    pub fn on_scroll(&mut self, direction: ScrollDirection) -> i32 {
        let now = Instant::now();
        let is_trackpad = self
            .last_event_at
            .is_some_and(|last| now.duration_since(last) < Duration::from_millis(35));
        self.last_event_at = Some(now);

        let lines_per_tick = if is_trackpad { 1 } else { 3 };
        direction.sign() * lines_per_tick
    }
}
