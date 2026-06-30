# Market Test Report — 2026-06-30 (DeepSeek)

## 测试配置

| 参数 | 值 |
|------|-----|
| 股票 | 贵州茅台 (600519.SH) |
| K线数量 | 60 根 |
| 辩论轮数 | 3 轮 |
| 风控讨论轮数 | 2 轮 |
| 模式 | debug_quick_only |
| LLM | DeepSeek (deepseek-chat) |

## 本次改进

| 改进项 | 文件 | 说明 |
|--------|------|------|
| has_execution_boundary 回退 | `scoring/helpers/technical.rs` | Trader说Hold时，回退到PM的confirmation_level/invalidation_level |
| blocking gaps 只用系统诊断 | `analysis/report_logic/setup_quality.rs` | 忽略LLM自填的blocking_gaps，只用diagnostics.availability/news |
| DeepSeek prompt cache | `llm/client/generation.rs` | 公共指令放入system message，所有请求共享前缀命中缓存 |
| system message 引导gap分类 | `llm/client/generation.rs` | 明确insider/earnings为tolerable，不是blocking |
| trader/portfolio 必填字段校验 | `llm/parse/validate.rs` | Buy/Sell必须有entry_price/stop_loss/time_horizon |
| trader/portfolio 重试逻辑 | `report/result/stages/finalize.rs` | 必填字段缺失时触发重试(最多3次) |

## 测试结果

### DeepSeek 贵州茅台

| 指标 | 值 |
|------|-----|
| 推荐 | Underweight (原始LLM: Underweight) |
| 置信度 | 61 |
| 方向分 | -44 (市场-12, 基本面-10, 舆情-9, 情绪-7, 风险-6) |
| 行动分 | 73 |
| 执行边界完整 | ✅ true |
| 交易设置质量 | execution_ready |
| Tokens | 47,572 |
| 请求数 | 21 |
| 耗时 | 129s |

### 关键指标

| 指标 | 值 |
|------|-----|
| 当前价 | 1176.30 |
| 60日高 | 1449.39 (距高点 -23.2%) |
| 60日低 | 1151.01 (距低点 +2.1%) |
| 上行概率 | 25% |
| 下行概率 | 45% |
| 横盘概率 | 30% |
| 确认位 | 1151.01 |
| 止损位 | 1286.99 |

### 技术指标

| 指标 | 值 | 状态 |
|------|-----|------|
| MA50 | 1286.99 | below_reference |
| EMA10 | 1195.95 | below_reference |
| MACD | -31.82 | bearish_cross |
| RSI | 39.14 | neutral |
| ADX | 24.11 | trend_moderate |
| ATR | 31.18 | normal_volatility |
| BOLL中轨 | 1221.23 | inside_band |
| CCI | -104.21 | oversold |

## 核心发现

### 1. execution_boundary_complete 修复 ✅

**问题**: 之前 `execution_boundary_complete` 始终为 false，原因有二：
1. `has_execution_boundary` 检查 trader 的 entry_price/stop_loss，但 Trader 说 Hold 时这些字段为空
2. LLM 把 insider transactions / earnings guidance 列为 blocking_gaps

**修复**:
1. `has_execution_boundary` 回退到 PM 的 confirmation_level/invalidation_level
2. `collect_execution_blocking_gaps` 只用系统诊断，忽略 LLM 自填的 blocking_gaps

**结果**: `执行边界完整: true`, `交易设置质量: execution_ready`

### 2. DeepSeek prompt cache 优化

**问题**: system message 只有 10 tokens，DeepSeek cache 需要前 64 tokens 匹配

**修复**: 公共指令（JSON格式、gap分类规则）放入 system message，所有请求共享

**效果**: 待观察 token 使用量变化

### 3. 方向分分解

| 维度 | 分数 |
|------|------|
| 市场 | -12 |
| 基本面 | -10 |
| 舆情 | -9 |
| 情绪 | -7 |
| 风险调整 | -6 |
| **总分** | **-44** |

### 4. 置信度分解

| 维度 | 分数 |
|------|------|
| 数据质量 | 19 |
| 趋势确认 | 25 |
| 基本面确认 | 25 |
| 催化剂 | 12 |
| 历史可迁移 | 0 |
| 跨代理一致 | 25 |
| 风险清晰 | 10 |
| **上限前总分** | **116** |
| **最终分** | **69** |
| **应用上限** | **88** |

## 与历史对比

| 指标 | 06-29 mimo | 06-30 DeepSeek(前) | 06-30 DeepSeek(后) |
|------|-----------|-------------------|-------------------|
| direction_score | -34 | -42 | -44 |
| confidence_score | 61 | 55 | 61 |
| execution_boundary | false | false | ✅ true |
| trade_setup | conditional | conditional | ✅ execution_ready |
| tokens | 110K | 42K | 47K |
| 耗时 | 1044s | 114s | 129s |

## 遗留问题

1. **历史可迁移始终为0** — 无历史校准数据
2. **action_score 偏低** — 73 (之前81)，原因待查
3. **盈亏比未显示** — 需检查 reward_risk_ratio 计算
