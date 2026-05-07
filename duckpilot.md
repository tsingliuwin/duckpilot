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
│   └── duck_cache.db       # DuckDB 持久化存储（Parquet 索引）
├── config.yaml             # 业务配置（行业背景、自定义公式、视图定义）
└── data/                   # 原始数据（Excel, CSV, Parquet）
```

### 2.2 逻辑层：四阶进化模型
1.  **环境感知 (Context Awareness)**: 自动扫描文件结构，提取 Schema。
2.  **语义层 (Semantic Layer)**: 将 `config.yaml` 中的规则映射为 DuckDB Macros 和 Views。
3.  **推理引擎 (Reasoning Engine)**: 基于 ReAct 模式的 NL2SQL 循环，支持自修复。
4.  **知识沉淀 (Knowledge Distillation)**: 异步提取对话中的业务逻辑，更新记忆库。


## 3. 关键特性细节

### 3.1 极致 DuckDB SQL 引擎
* **语法优化**: 强制 LLM 使用 DuckDB 现代 SQL（如 `ASOF JOIN` 处理时序，`PIVOT` 处理报表，`SELECT * EXCLUDE` 处理宽表）。
* **流式处理**: 直接对 Excel 文件进行谓词下推（Predicate Pushdown），无需全量载入内存。
* **宏化逻辑**: 自动将业务指标（如“复购率”、“毛利”）编译为 SQL Macro，提高生成准确度。

### 3.2 Pilot's Log：自我进化记忆系统
* **碎片采集**: 监听用户纠正（如“不，这里的利润要扣除税点”）。
* **向量检索**: 使用轻量级向量库（如 LanceDB）存储业务定义。在 NL2SQL 阶段前，自动检索并注入相关背景。
* **反思机制**: 周期性调用轻量 LLM 对 `pilot_log` 进行归纳，更新 `config.yaml` 中的静态规则。

### 3.3 声明式数据清洗
* **非破坏性**: 原始数据只读，通过 DuckDB 视图实现清洗（填补、去重、格式化）。
* **配置化驱动**: 清洗规则记录在 `config.yaml`，用户可通过自然语言随时修改规则。


## 4. 技术栈选型

| 维度 | 组件 | 选择理由 |
| :--- | :--- | :--- |
| **内核语言** | **Rust** | 高性能、内存安全、易于分发二进制工具。 |
| **计算引擎** | **DuckDB** | OLAP 性能极强，原生支持 Excel，SQL 语法现代。 |
| **LLM 适配** | **OpenAI API Standard** | 确保兼容 GPT-4o、Claude、DeepSeek 以及 Ollama。 |
| **向量数据库** | **LanceDB (Embedded)** | 无需独立服务器，与 Rust 集成良好，适合本地存储。 |
| **配置管理** | **Serde (Rust)** | 快速解析 JSON/YAML 配置文件。 |
| **CLI 交互** | **Clap / Inquire** | 打造工业级易用的命令行交互体验。 |


## 5. 工作流详解

### Step 1: 激活 (`duckpilot init`)
1.  **全局检查**: 读取 `~/.duckpilot/settings.json` 获取 API 密钥。
2.  **数据体检**: 扫描文件夹内所有 Excel，生成 Schema 快照。
3.  **画像构建**: 引导用户回答行业、分析角色（写入 `config.yaml`）。
4.  **历史追溯 (Bonus)**: 预读已有的分析文档/脚本，预填充 `pilot_log`。

### Step 2: 理解与清洗 (`duckpilot clean`)
1.  Agent 提出清洗建议（如：“发现日期列格式不一，是否统一？”）。
2.  用户确认，规则写入 `config.yaml`。

### Step 3: 循环分析 (`duckpilot chat`)
1.  **用户**: “对比上月，哪个品类退货率激增？”
2.  **Agent**: 检索记忆 -> 生成 DuckDB SQL -> 执行 -> 发现异常。
3.  **反馈**: “发现‘户外用品’激增 15%，主因是 A 供应商。是否记录此分析逻辑？”


## 6. 路线图 (Roadmap)

### 第一阶段：MVP (最小可行性产品)
* [ ] 实现 Rust 基础架构与配置管理（Global vs Project）。
* [ ] 集成 DuckDB 基础查询功能。
* [ ] 跑通基础的 NL2SQL（支持 Schema 注入）。

### 第二阶段：Intelligence (智能增强)
* [ ] 开发 **Pilot's Log** 记忆系统。
* [ ] 支持 SQL 报错自动修复循环。
* [ ] 实现声明式清洗逻辑（View-based）。

### 第三阶段：Ecosystem (生态扩展)
* [ ] 支持生成可视化图表（通过静态 HTML/Plotly）。
* [ ] 适配 **MCP (Model Context Protocol)** 协议。
* [ ] 支持导出完整的 Markdown 业务报告。


## 7. 结语
**DuckPilot** 不仅仅是一个 SQL 生成器，它是数据分析师的**数字孪生**。它通过不断吸收用户的业务知识，最终将实现从“工具”到“伙伴”的跨越。
