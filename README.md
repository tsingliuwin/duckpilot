# 🛸 DuckPilot

**DuckPilot** 是一款基于 **Rust** 开发、以 **DuckDB** 为计算引擎、通过 **LLM** 驱动的本地数据分析 Agent。它旨在解决大模型处理大数据时的 Token 瓶颈与隐私问题，通过“极致 DuckDB 化”的 SQL 生成、**DuckLake 本地湖仓一体架构**和“自我进化”的业务记忆系统，为用户提供从原始数据（如 Excel, CSV, Parquet）到深度商业洞察的端到端体验。

## 🌟 核心哲学

- **Local First (本地优先)**: 数据不离本地，计算不走云端，保护业务数据隐私。
- **Extreme DuckDB (极致性能)**: 充分压榨 DuckDB 向量化执行引擎的性能，中间过程用户无感。
- **Incremental Intelligence (进化智能)**: 像老员工一样，在持续的对话与分析中逐渐理解业务细节与逻辑。

## ✨ 主要特性

- **DuckLake 湖仓一体**: 透明地将 CSV、Excel、Parquet 等原始数据文件自动摄入为轻量级本地高性能 **DuckLake** 格式，支持并发安全、ACID 和谓词下推，全程无需用户干预。
- **NL2SQL 自然语言交互**: 结合大语言模型（支持 GPT-4o, Claude, DeepSeek, Ollama 等），根据本地数据 Schema 自动生成并执行 DuckDB SQL。
- **全屏终端界面 (TUI)**: 基于 `ratatui` 构建的专业四面板交互界面，支持数据结构预览、对话、表格查看与分屏显示。
- **声明式配置**: 业务线、数据清洗规则与自定义逻辑可通过 `config.yaml` 灵活配置。
- **异步事件驱动**: 采用 `tokio` 异步运行时架构，确保 I/O 密集型任务与界面渲染互不阻塞。

## 🛠️ 技术栈

- **语言**: Rust 🦀
- **TUI 框架**: ratatui, crossterm
- **计算引擎**: DuckDB
- **存储架构**: DuckLake (轻量级本地湖仓)
- **LLM 交互**: reqwest, serde_json
- **异步运行时**: tokio

## 🚀 快速开始

### 前置要求

- 安装了 [Rust 工具链](https://rustup.rs/) (edition 2024)。

### 构建与运行

1. 克隆本项目并进入目录：
   ```bash
   git clone <your-repo-url>
   cd duckpilot
   ```

2. 编译并运行：
   ```bash
   cargo run --release
   ```

### 界面与交互

进入 TUI 界面后，您可以使用以下方式与 DuckPilot 交互：

- **焦点切换**: 使用 `Tab` / `Shift+Tab` 在输入框、对话区、数据结构区、表格区之间切换焦点。
- **快捷指令** (在输入框中使用):
  - `/clear` — 清空对话历史。
  - `/refresh` — 重新扫描工作区数据文件并刷新表结构。
  - `/chat` — 切换为仅显示对话模式。
  - `/table` — 切换为仅显示查询表格模式。
  - `/split` — 切换为对话与表格分屏模式。
  - `/quit` 或 `/exit` 或 `/q` — 退出程序。

## 🗺️ 项目路线图 (Roadmap)

### 第一阶段：MVP [进行中]
- [x] Rust 基础架构与 TUI 框架搭建。
- [x] 集成 DuckDB 引擎与文件自动发现。
- [ ] 跑通基础的自然语言转 SQL (NL2SQL) 工作流。

### 第二阶段：智能增强 (Intelligence)
- [ ] 开发 Pilot's Log 自我进化记忆系统。
- [ ] 支持 SQL 执行报错后的 LLM 自动修复。
- [ ] 基于声明式的清洗与视图生成逻辑。

### 第三阶段：生态扩展 (Ecosystem)
- [ ] 终端内轻量级可视化图表支持。
- [ ] 适配 MCP (Model Context Protocol) 协议。
- [ ] 自动导出 Markdown 格式的商业分析报告。

## 📄 许可证

本项目基于 [MIT License](LICENSE) 许可开源。
