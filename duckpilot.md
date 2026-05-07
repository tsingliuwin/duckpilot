# 🛸 DuckPilot 项目方案白皮书 (v1.0)

## 1. 项目定位
**DuckPilot** 是一款基于 **Rust** 开发、以 **DuckDB** 为计算引擎、通过 **LLM** 驱动的本地数据分析 Agent。它旨在解决大模型处理大数据时的 Token 瓶颈与隐私问题，通过“极致 DuckDB 化”的 SQL 生成和“自我进化”的业务记忆系统，为用户提供从原始 Excel 到深度商业洞察的端到端体验。

### 核心哲学：
* **Local First**: 数据不离本地，计算不走云端。
* **Extreme DuckDB**: 性能压榨到极致，中间过程用户无感。
* **Incremental Intelligence**: 像老员工一样，在陪伴中逐渐理解业务细节。


## 2. 核心架构设计

### 2.1 物理层：项目空间 (Project Space)
每个分析项目是一个独立的文件夹，包含隐藏的“大脑”目录：
```text
[Project_Root]/
├── .duckpilot/
│   ├── settings.json       # 技术配置（模型、API、线程限制等）
│   ├── pilot_log/          # 记忆系统（本地向量索引 & 对话快照）
│   ├── metadata.ducklake        # DuckLake 元数据（Catalog）
│   └── metadata.ducklake.files/ # DuckLake Parquet 数据池
├── config.yaml             # 业务配置（行业背景、自定义公式、视图定义）
└── data/                   # 原始数据（Excel, CSV, Parquet）
```

### 2.2 逻辑层：四阶进化模型
1.  **环境感知 (Context Awareness)**: 自动扫描文件结构，提取 Schema。
2.  **语义层 (Semantic Layer)**: 将 `config.yaml` 中的规则映射为 DuckDB Macros 和 Views。
3.  **推理引擎 (Reasoning Engine)**: 基于 ReAct 模式的 NL2SQL 循环，支持自修复。
4.  **知识沉淀 (Knowledge Distillation)**: 异步提取对话中的业务逻辑，更新记忆库。


### 2.3 运行时架构：事件驱动模型
DuckPilot 采用 **tokio 异步运行时 + mpsc 事件通道** 的架构，所有 I/O 密集操作（LLM 调用、SQL 执行、文件扫描）均在后台任务中完成，通过事件通道回传结果，主循环只做事件分发与 UI 渲染。

```
┌─────────────── 主循环 ──────────────────┐
│  terminal.draw(render)                   │
│  events.next().await → handle_event()    │
└──────────┬──────────────────┬────────────┘
           │                  │
    tokio::spawn        tokio::spawn
           │                  │
    ┌──────▼──────┐   ┌──────▼──────┐
    │ LLM 流式调用 │   │ SQL 执行     │
    │ 文件扫描     │   │ DuckLake IO  │
    └──────┬──────┘   └──────┬──────┘
           │                  │
      AppEvent::LlmChunk  AppEvent::QueryResult
      AppEvent::LlmDone   AppEvent::SchemaDone
      AppEvent::LlmError  AppEvent::QueryError
           │                  │
           └───── mpsc ──────┘
```

核心事件类型：
* **终端事件**: Key / Mouse / Resize / Tick（250ms 心跳）
* **LLM 事件**: LlmChunk（流式片段）/ LlmDone / LlmError
* **引擎事件**: QueryResult / QueryError / SchemaDone

DuckDB 的 `Connection` 不是 `Send`，通过 `Arc<Mutex<DbEngine>>` 保护，确保同一时刻只有一个异步任务持有连接。


## 3. 关键特性细节

### 3.1 极致 DuckDB & DuckLake SQL 引擎
* **湖仓一体**: 自动将 CSV、Excel、Parquet 统一摄入为 **DuckLake** 格式，实现多过程并发安全与高性能存储。
* **语法优化**: 强制 LLM 使用 DuckDB 现代 SQL（如 `ASOF JOIN` 处理时序，`PIVOT` 处理报表，`SELECT * EXCLUDE` 处理宽表）。
* **透明摄入**: 直接对原始文件进行谓词下推（Predicate Pushdown），自动转换为 DuckLake 表，无需用户干预。

### 3.2 Pilot's Log：自我进化记忆系统
* **碎片采集**: 监听用户纠正（如“不，这里的利润要扣除税点”）。
* **向量检索**: 使用轻量级向量库（如 LanceDB）存储业务定义。在 NL2SQL 阶段前，自动检索并注入相关背景。
* **反思机制**: 周期性调用轻量 LLM 对 `pilot_log` 进行归纳，更新 `config.yaml` 中的静态规则。

### 3.3 声明式数据清洗与湖仓转换
* **自动化摄入**: 原始数据只读，通过 DuckLake 自动转换为高性能的本地 Parquet 湖。
* **配置化驱动**: 清洗规则记录在 `config.yaml`，用户可通过自然语言随时修改规则，系统自动重新生成 DuckLake 视图或表。

### 3.4 TUI 交互设计
DuckPilot 提供一个全屏终端 UI，基于 **ratatui + crossterm** 构建，采用四面板布局：

```
┌─────────── 标题栏 ──────────────────────────────┐
│ 🛸 DuckPilot v0.1  │ 📁 项目名  │ 📡 模型名     │
├────────────┬─────────────────────────────────────┤
│            │                                     │
│  📊 数据结构 │          💬 对话 / 📋 查询结果       │
│  (Schema)  │                                     │
│  可展开的   │   根据 ViewMode 切换:               │
│  表/列树    │   F1-Chat  F2-Table  F3-Split      │
│            │                                     │
├────────────┴─────────────────────────────────────┤
│ ✏️ 输入 (Enter 发送 | Esc 退出)                    │
├──────────────────────────────────────────────────┤
│ DB:✓ │ LLM:✓ │ gpt-4o │ 📁 N个数据文件 │ 快捷键  │
└──────────────────────────────────────────────────┘
```

**焦点管理**: `Tab` / `Shift+Tab` 在四个区域间循环切换（Input → Chat → Schema → Table），焦点区域边框高亮。

**命令系统**: 输入框中以 `/` 开头的命令直接由 App 处理，不发送给 LLM：
* `/clear` — 清空对话历史
* `/refresh` — 重新扫描 data/ 目录并刷新表结构
* `/chat` `/table` `/split` — 切换右侧视图模式
* `/quit` `/exit` `/q` — 退出

**输入框**: 支持光标移动（←/→/Home/End）、退格删除、历史记录浏览（↑/↓）、`Ctrl+U` 清空行，并正确处理中文字符宽度。

### 3.5 NL2SQL 流水线
用户输入自然语言后的完整处理流程：

```
用户输入 "哪个品类退货率最高?"
        │
        ▼
┌─ 构造 System Prompt ─────────────┐
│ 1. DuckDB SQL 角色设定           │
│ 2. 约束：只输出 SQL，不解释       │
│ 3. 注入当前所有表的 Schema 信息    │
│    （表名、列名、类型、可空性）     │
└──────────────┬──────────────────┘
               ▼
      OpenAI API (stream=true)
      兼容 DeepSeek / Ollama 等
               │
      ┌────────┴────────┐
      │  流式回调        │
      │  AppEvent::      │
      │  LlmChunk → 实时 │
      │  显示在 Chat 面板 │
      └────────┬────────┘
               ▼
        LlmDone 事件
               │
      ┌────────┴────────┐
      │ extract_sql()    │
      │ 解析 LLM 返回的  │
      │ Markdown 代码块  │
      │ ```sql ... ```   │
      └────────┬────────┘
               ▼
     有 SQL 且非注释? ──No──→ 仅展示文本
          │ Yes
          ▼
    在 DuckLake 中执行 SQL
          │
    ┌─────┴──────┐
    │ QueryResult │ → 表格视图展示 + 摘要信息
    │ QueryError  │ → 错误提示（未来接入自修复）
    └────────────┘
```

**System Prompt 策略**: 将所有已扫描表的完整 Schema（表名、列名、数据类型、是否可空）注入，指导 LLM 生成符合 DuckDB 语法的 SQL。若无法转换则返回 `--` 开头的说明。

**SQL 提取逻辑**: LLM 通常返回包含 Markdown 代码块的文本（\`\`\`sql ... \`\`\`），`extract_sql()` 负责提取其中的纯 SQL。若 LLM 直接返回 SQL（无代码块包裹），则整体视为 SQL。

**当前限制**: 单轮对话，不向 LLM 传递历史消息，暂不支持追问。未来计划在第二阶段引入多轮上下文。


## 4. 技术栈选型

| 维度 | 组件 | 选择理由 |
| :--- | :--- | :--- |
| **内核语言** | **Rust** | 高性能、内存安全、易于分发二进制工具。 |
| **构建系统** | **Cargo + build.rs** | Windows 平台链接 `Rstrtmgr` 库用于进程管理。 |
| **计算引擎** | **DuckDB** | OLAP 性能极强，SQL 语法现代。 |
| **存储层** | **DuckLake** | 轻量级本地湖仓格式，支持 ACID、并发与时间旅行。 |
| **LLM 适配** | **OpenAI API Standard** | 确保兼容 GPT-4o、Claude、DeepSeek 以及 Ollama。 |
| **向量数据库** | **LanceDB (Embedded)** | 无需独立服务器，与 Rust 集成良好，适合本地存储。 |
| **配置管理** | **Serde (Rust)** | 快速解析 JSON/YAML 配置文件。 |
| **CLI 交互** | **Clap / Ratatui** | 打造工业级易用的命令行与 TUI 交互体验。 |


## 5. 工作流详解

### Step 1: 激活 (`duckpilot init`)
1.  **全局检查**: 读取 `~/.duckpilot/settings.json` 获取 API 密钥。
2.  **数据体检**: 扫描文件夹内所有数据文件，自动创建 DuckLake Catalog。
3.  **画像构建**: 引导用户回答行业、分析角色（写入 `config.yaml`）。
4.  **历史追溯 (Bonus)**: 预读已有的分析文档/脚本，预填充 `pilot_log`。

### Step 2: 理解与清洗 (`duckpilot clean`)
1.  Agent 提出清洗建议（如：“发现日期列格式不一，是否统一？”）。
2.  用户确认，规则写入 `config.yaml`，系统自动更新 DuckLake 中的清洗逻辑。

### Step 3: 循环分析 (`duckpilot chat`)
1.  **用户**: “对比上月，哪个品类退货率激增？”
2.  **Agent**: 检索记忆 -> 生成 DuckDB SQL -> 在 DuckLake 引擎执行 -> 发现异常。
3.  **反馈**: “发现‘户外用品’激增 15%，主因是 A 供应商。是否记录此分析逻辑？”


## 6. 路线图 (Roadmap)

### 第一阶段：MVP (最小可行性产品) [进行中]
* [x] 实现 Rust 基础架构与配置管理（Global vs Project）。
* [x] 集成 DuckDB + DuckLake 湖仓一体化查询引擎。
* [x] 修复 Excel (v1.2+) 兼容性问题。
* [ ] 跑通基础的 NL2SQL（支持 Schema 注入）。

### 第二阶段：Intelligence (智能增强)
* [ ] 开发 **Pilot's Log** 记忆系统。
* [ ] 支持 SQL 报错自动修复循环。
* [ ] 实现声明式清洗逻辑（DuckLake Table/View-based）。

### 第三阶段：Ecosystem (生态扩展)
* [ ] 支持生成可视化图表（通过静态 HTML/Plotly）。
* [ ] 适配 **MCP (Model Context Protocol)** 协议。
* [ ] 支持导出完整的 Markdown 业务报告。


## 7. 结语
**DuckPilot** 不仅仅是一个 SQL 生成器，它是数据分析师的**数字孪生**。它通过不断吸收用户的业务知识，最终将实现从“工具”到“伙伴”的跨越。
