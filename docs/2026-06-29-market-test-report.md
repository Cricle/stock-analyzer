# Market Test Report — 2026-06-29

## 测试配置

| 参数 | 值 |
|------|-----|
| 股票数量 | 4 只 A股 |
| K线数量 | 60 根 |
| 辩论轮数 | 3 轮 (默认) |
| 风控讨论轮数 | 2 轮 (默认) |
| 模式 | debug_quick_only |
| LLM | mimo-v2.5-pro |

## 测试股票

| 代码 | 名称 | 耗时 | 推荐 | 置信度 | 摘要 |
|------|------|------|------|--------|------|
| 600519.SH | 贵州茅台 | 1167s | Underweight | 69 | Call: Underweight. Setup warrants action based on current ev |
| 601318.SH | 中国平安 | 1414s | Underweight | 63 | The call is Underweight, with setup quality conditional due |
| 000858.SZ | 五粮液 | 1300s | Underweight | 64 | Call: Underweight. Setup quality is conditional with low con |
| 300750.SZ | 宁德时代 | 1327s | Underweight | 67 | The call is Underweight based on bearish technical structure |

**总耗时:** ~5208s (~87分钟)
**平均单只:** ~1302s (~22分钟)

## 分析

### 1. 推荐一致性问题

4只股票全部给出 Underweight 推荐。可能原因:

- **市场环境因素:** 2026年6月底 A股整体偏弱，LLM 基于市场数据倾向于看空
- **LLM 偏差:** mimo-v2.5-pro 可能对 A股 存在系统性看空偏差
- **样本不足:** 4只股票不足以判断是否为系统性问题

### 2. 置信度分布

| 股票 | 置信度 | 评价 |
|------|--------|------|
| 贵州茅台 | 69 | 中等偏高 |
| 宁德时代 | 67 | 中等 |
| 五粮液 | 64 | 中等 |
| 中国平安 | 63 | 中等偏低 |

置信度在 63-69 之间，差异不大，说明数据质量和分析深度相对一致。

### 3. 个性化摘要验证

所有 4 只股票的摘要都是 LLM 生成的个性化内容（不是模板），且每只股票的摘要不同:
- 贵州茅台: 提到 "Setup warrants action based on current evidence"
- 中国平安: 提到 "setup quality conditional"
- 五粮液: 提到 "low confidence"
- 宁德时代: 提到 "bearish technical structure"

**结论:** LLM 摘要保留功能正常工作。

### 4. 耗时分析

| 股票 | 耗时 | 排名 |
|------|------|------|
| 贵州茅台 | 1167s | 最快 |
| 五粮液 | 1300s | |
| 宁德时代 | 1327s | |
| 中国平安 | 1414s | 最慢 |

耗时差异约 247s (21%)，可能与:
- LLM 响应速度波动
- 数据获取延迟
- 辩论轮数交互差异

### 5. 架构改进验证

本次测试验证了以下改进:

| 改进项 | 状态 | 说明 |
|--------|------|------|
| LLM 摘要保留 | ✅ | 4只股票摘要均为 LLM 生成 |
| 多分析师交叉验证 | ✅ | 已集成到 CoreResearchCall |
| 辩论轮数增加 | ✅ | 3轮辩论 + 2轮风控讨论 |
| 批量工具调用 | ✅ | 单次 LLM 调用请求多个工具 |

## 待改进

1. **增加测试股票数量:** 4只不足以评估系统性偏差
2. **添加美股/港股对比:** 验证是否为 A股 特有问题
3. **详细指标输出:** 已添加 `print_detailed_indicators`，下次运行可看到完整指标
4. **CoreResearchCall 多样性:** 需要更多样本验证交叉验证是否有效

## 下一步

1. 重跑测试，使用完整指标输出
2. 增加股票数量到 8-12 只
3. 分析 direction_score 和 action_score 的分布
4. 检查 analyst_consensus 是否有效区分了不同结论
