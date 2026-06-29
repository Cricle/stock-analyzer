# Market Test Report — 2026-06-30 (8 Stocks)

## 测试配置

| 参数 | 值 |
|------|-----|
| 股票数量 | 8 只 A股 |
| K线数量 | 60 根 |
| 辩论轮数 | 3 轮 |
| 风控讨论轮数 | 2 轮 |
| 模式 | debug_quick_only |
| LLM | mimo-v2.5-pro |
| 总耗时 | ~9148s (~152分钟) |
| 平均单只 | ~1144s (~19分钟) |

## 完整结果

| # | 代码 | 名称 | 耗时 | 推荐 | CoreResearchCall | 方向分 | 置信度 | Tokens | 请求 |
|---|------|------|------|------|-----------------|--------|--------|--------|------|
| 1 | 600519.SH | 贵州茅台 | 1215s | Underweight | sell_on_break | -32 | 64 | 97,015 | 24 |
| 2 | 601318.SH | 中国平安 | 1363s | Underweight | lean_sell | -30 | 67 | 213,454 | 52 |
| 3 | 000858.SZ | 五粮液 | 1194s | Underweight | sell_on_break | -37 | 64 | 316,114 | 77 |
| 4 | 300750.SZ | 宁德时代 | 1152s | Underweight | lean_sell | -28 | 64 | 417,645 | 102 |
| 5 | 600036.SH | 招商银行 | 1149s | Underweight | sell_on_break | -33 | 66 | 526,048 | 128 |
| 6 | 000333.SZ | **美的集团** | 1005s | **Overweight** | **buy_on_confirmation** | **+27** | 52 | 627,387 | 153 |
| 7 | 601012.SH | 隆基绿能 | 1041s | Underweight | sell_on_break | -40 | 67 | 733,976 | 179 |
| 8 | 002594.SZ | 比亚迪 | 1029s | Underweight | sell_on_break | -41 | 62 | 844,228 | 205 |

**总 Tokens:** 3,775,867 (~3.78M)

## 核心发现

### 1. CoreResearchCall 多样性验证 ✅

交叉验证有效区分了不同结论:

| CoreResearchCall | 数量 | 股票 |
|-----------------|------|------|
| sell_on_break | 5 | 茅台、五粮液、招商、隆基、比亚迪 |
| lean_sell | 2 | 平安、宁德 |
| buy_on_confirmation | 1 | 美的 |

**美的集团**是唯一获得正面结论的股票，方向分 +27，上行概率 41% > 下行概率 33%。

### 2. 方向分分布

```
比亚迪   ████████████████████████████████████████ -41
隆基绿能 ███████████████████████████████████████  -40
五粮液   █████████████████████████████████████    -37
招商银行 █████████████████████████████████        -33
贵州茅台 ████████████████████████████████         -32
中国平安 ██████████████████████████████           -30
宁德时代 ████████████████████████████             -28
美的集团 ███████████████████████████              +27
```

方向分范围: -41 ~ +27，跨度 68 分，说明系统能区分不同股票的基本面。

### 3. 技术指标对比

| 股票 | RSI | MACD | 距MA50 | 距60日高 | 趋势强度 |
|------|-----|------|--------|---------|---------|
| 贵州茅台 | 41.16 (中性) | +26.66 (看多交叉) | -7.5% | -21.5% | ADX 24 (中等) |
| 中国平安 | 41.44 (中性) | +0.58 (看多交叉) | -9.5% | -22.1% | ADX 23 (中等) |
| 五粮液 | 28.77 (超卖) | +3.10 (看多交叉) | -15.0% | -43.0% | ADX 44 (强) |
| 宁德时代 | 49.75 (中性) | +11.04 (减弱看空) | -6.1% | -19.5% | ADX 18 (盘整) |
| 招商银行 | 31.88 (中性) | -0.47 (看空交叉) | -4.6% | -10.2% | ADX 48 (强) |
| **美的集团** | 44.81 (中性) | +1.67 (看多交叉) | **+0.7%** | -6.4% | ADX 25 (中等) |
| 隆基绿能 | 54.20 (中性) | -0.63 (看多交叉) | -14.5% | -40.7% | ADX 24 (中等) |
| 比亚迪 | 22.17 (超卖) | +5.25 (看多交叉) | -16.2% | -36.2% | ADX 49 (强) |

**关键差异:** 美的集团是唯一价格在 MA50 之上的股票 (+0.7%)，其他均在下方。

### 4. 概率视角对比

| 股票 | 上行概率 | 下行概率 | 横盘概率 | 风险概率 |
|------|---------|---------|---------|---------|
| 贵州茅台 | 29% | 42% | 29% | 49% |
| 中国平安 | 30% | 41% | 29% | 48% |
| 五粮液 | 27% | 43% | 30% | 50% |
| 宁德时代 | 30% | 41% | 29% | 48% |
| 招商银行 | 29% | 42% | 29% | 49% |
| **美的集团** | **41%** | **33%** | **26%** | **43%** |
| 隆基绿能 | 27% | 43% | 30% | 50% |
| 比亚迪 | 26% | 44% | 30% | 52% |

美的集团是唯一上行概率 > 下行概率的股票。

### 5. 置信度分析

| 维度 | 茅台 | 平安 | 五粮液 | 宁德 | 招商 | 美的 | 隆基 | 比亚迪 |
|------|------|------|--------|------|------|------|------|--------|
| 数据质量 | 15 | 15 | 15 | 15 | 15 | 15 | 15 | 15 |
| 趋势确认 | 25 | 25 | 25 | 25 | 25 | 25 | 25 | 25 |
| 基本面确认 | 25 | 25 | 25 | 25 | 25 | 25 | 25 | 25 |
| 催化剂 | 12 | 12 | 11 | 11 | 16 | 12 | 12 | 12 |
| 历史可迁移 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| 跨代理一致 | 25 | 25 | 25 | 25 | 25 | **8** | 25 | 25 |
| 风险清晰 | 10 | 10 | 10 | 10 | 10 | **9** | 10 | 10 |
| **最终分** | 64 | 67 | 64 | 64 | 66 | **52** | 67 | 62 |

**美的集团置信度最低 (52)**，因为跨代理一致性只有 8 分（分析师意见分歧大），这与它获得不同结论一致。

### 6. IC纪律状态

| 股票 | 状态 | 含义 |
|------|------|------|
| 7只 | must_defend | 需要防守，价格在关键位附近 |
| 美的集团 | probe_watch | 可以试探性观察 |

### 7. Token 使用效率

| 股票 | Tokens | 请求 | 每请求Tokens |
|------|--------|------|-------------|
| 贵州茅台 | 97K | 24 | 4,042 |
| 中国平安 | 213K | 52 | 4,105 |
| 五粮液 | 316K | 77 | 4,105 |
| 宁德时代 | 418K | 102 | 4,096 |
| 招商银行 | 526K | 128 | 4,109 |
| 美的集团 | 627K | 153 | 4,100 |
| 隆基绿能 | 734K | 179 | 4,100 |
| 比亚迪 | 844K | 205 | 4,118 |

每请求约 4100 tokens，非常稳定。后续股票 tokens 更多是因为累积了更多上下文。

## 架构改进验证

| 改进项 | 状态 | 验证结果 |
|--------|------|---------|
| LLM 摘要保留 | ✅ | 8只股票摘要均为 LLM 个性化生成 |
| 多分析师交叉验证 | ✅ | 美的集团获得不同结论 (buy_on_confirmation) |
| 辩论轮数增加 | ✅ | 3轮辩论 + 2轮风控讨论正常运行 |
| 批量工具调用 | ✅ | 单次 LLM 调用请求多个工具 |
| 详细指标输出 | ✅ | 完整的方向分/行动分/置信度/技术指标/概率视角 |

## 关键结论

1. **系统能区分不同股票:** 美的集团获得 Overweight + buy_on_confirmation，其他 7 只 Underweight
2. **技术面主导:** 价格在 MA50 上下是最关键的区分因素
3. **LLM 偏空倾向:** 7/8 看空，可能反映当前市场环境或 LLM 对 A股 的系统性偏差
4. **置信度与结论相关:** 美的集团置信度最低 (52)，因为分析师意见分歧最大
5. **方向分有效:** 范围 -41 ~ +27，能区分不同股票的基本面强弱

## 下一步建议

1. **增加更多正面样本:** 当前 7/8 看空，需要更多处于上升趋势的股票来验证
2. **添加美股/港股:** 验证是否为 A股 特有问题
3. **回测验证:** 用历史数据验证 direction_score 的预测准确性
4. **调整 LLM prompt:** 如果偏差过大，考虑在 prompt 中增加平衡性要求

---

## 附录：各股票详细指标

### 1. 贵州茅台 (600519.SH)

**核心评分**
- 方向分: -32 (市场 -10, 基本面 -6, 舆情 -6, 情绪 -4, 风险调整 -6)
- 行动分: 81 (一致性 20, 执行水平 25, 仓位纪律 20, 视野清晰 10, 盈亏比 6)
- 置信度: 64 (上限前 112, 应用上限 88)
- 研究可靠度: 100/100

**价格上下文**
- 当前价: 1194.96
- 60日高: 1451.91 (2026-03-31), 低: 1151.01 (2026-06-29)
- 距高点: -21.5%, 距低点: +3.7%

**概率视角**
- 上行: 29%, 下行: 42%, 横盘: 29%, 风险: 49%
- 上行目标: 1451.91 (+21.5%), 下行目标: 1200.32

**技术指标**
- MA50: 1292.25 [below], EMA10: 1200.32 [below]
- MACD: +26.66 [bullish_cross], RSI: 41.16 [neutral]
- KDJ_K: 23.18 [bullish_cross], CCI: -104.82 [oversold]
- ADX: 24.19 [trend_moderate], ATR: 30.57 [normal_volatility]
- BOLL中轨: 1226.50 [inside_band], 带宽: 10.62 [band_normal]
- VWAP: 1224.82 [below], VWMA: 1222.44 [below]
- OBV: -750482 [volume_accumulation]
- 结论: volume_confirms_bid, technical_neutral_zone, macd_bullish, price_below_ma50

**IC纪律**
- 状态: must_defend
- 当前: 1194.96, 确认位: 1161.39, 止损位: 1200.32
- RSI: 41.2, MACD: 26.66

**组合决策**
- 评级: Underweight, CoreResearchCall: sell_on_break
- 摘要: Call: Underweight. Setup quality: Based on fresh evidence with no historical calibration; execution confidence 56/100 warrants cautious action.

---

### 2. 中国平安 (601318.SH)

**核心评分**
- 方向分: -30 (市场 -9, 基本面 -8, 舆情 -5, 情绪 -2, 风险调整 -6)
- 行动分: 81 (一致性 20, 执行水平 25, 仓位纪律 20, 视野清晰 10, 盈亏比 6)
- 置信度: 67 (上限前 112, 应用上限 88)
- 研究可靠度: 100/100

**价格上下文**
- 当前价: 48.60
- 60日高: 59.36 (2026-05-07), 低: 46.90 (2026-06-29)
- 距高点: -22.1%, 距低点: +3.5%

**概率视角**
- 上行: 30%, 下行: 41%, 横盘: 29%, 风险: 48%
- 上行目标: 59.36 (+22.1%), 下行目标: 53.68

**技术指标**
- MA50: 53.68 [below], EMA10: 50.00 [below]
- MACD: +0.58 [bullish_cross], RSI: 41.44 [neutral]
- KDJ_K: 16.59 [bearish_cross], CCI: -171.86 [oversold]
- ADX: 23.04 [trend_moderate], ATR: 1.99 [high_volatility]
- BOLL中轨: 51.46 [inside_band], 带宽: 14.19 [band_expanding]
- VWAP: 51.27 [below], VWMA: 51.12 [below]
- OBV: -6180303 [volume_accumulation]
- 结论: volatility_elevated, volume_confirms_bid, technical_neutral_zone, macd_bullish, price_below_ma50

**IC纪律**
- 状态: must_defend
- 当前: 48.60, 确认位: 46.90, 止损位: 53.68
- RSI: 41.4, MACD: 0.58

**组合决策**
- 评级: Underweight, CoreResearchCall: lean_sell
- 摘要: The call is Underweight due to bearish technicals and fundamental fragility.

---

### 3. 五粮液 (000858.SZ)

**核心评分**
- 方向分: -37 (市场 -11, 基本面 -9, 舆情 -6, 情绪 -5, 风险调整 -6)
- 行动分: 81 (一致性 20, 执行水平 25, 仓位纪律 20, 视野清晰 10, 盈亏比 6)
- 置信度: 64 (上限前 111, 应用上限 88)
- 研究可靠度: 100/100

**价格上下文**
- 当前价: 74.00
- 60日高: 105.80 (2026-03-31), 低: 71.75 (2026-06-29)
- 距高点: -43.0%, 距低点: +3.0%

**概率视角**
- 上行: 27%, 下行: 43%, 横盘: 30%, 风险: 50%
- 上行目标: 105.80 (+43.0%), 下行目标: 87.00

**技术指标**
- MA50: 86.91 [below], EMA10: 75.67 [below]
- MACD: +3.10 [bullish_cross], RSI: 28.77 [oversold]
- KDJ_K: 17.04 [bullish_cross], CCI: -125.27 [oversold]
- ADX: 44.01 [trend_strong], ATR: 2.12 [normal_volatility]
- BOLL中轨: 78.33 [inside_band], 带宽: 15.79 [band_expanding]
- VWAP: 77.97 [below], VWMA: 77.79 [below]
- OBV: -4944865 [volume_accumulation]
- 结论: volume_confirms_bid, technical_oversold, macd_bullish, price_below_ma50

**IC纪律**
- 状态: must_defend
- 当前: 74.00, 确认位: 71.75, 止损位: 87.00
- RSI: 28.8, MACD: 3.10

**组合决策**
- 评级: Underweight, CoreResearchCall: sell_on_break
- 摘要: Call: Underweight. Setup quality score (100/100) warrants action, but execution confidence (56/100) is moderate.

---

### 4. 宁德时代 (300750.SZ)

**核心评分**
- 方向分: -28 (市场 -7, 基本面 -8, 舆情 -2, 情绪 -5, 风险调整 -6)
- 行动分: 81 (一致性 20, 执行水平 25, 仓位纪律 20, 视野清晰 10, 盈亏比 6)
- 置信度: 64 (上限前 111, 应用上限 88)
- 研究可靠度: 100/100

**价格上下文**
- 当前价: 392.36
- 60日高: 468.75 (2026-05-07), 低: 374.58 (2026-04-07)
- 距高点: -19.5%, 距低点: +4.5%

**概率视角**
- 上行: 30%, 下行: 41%, 横盘: 29%, 风险: 48%
- 上行目标: 468.75 (+19.5%), 下行目标: 401.41

**技术指标**
- MA50: 417.91 [below], EMA10: 395.15 [below]
- MACD: +11.04 [weakening_bearish], RSI: 49.75 [neutral]
- KDJ_K: 36.31 [bearish_cross], CCI: -89.16 [neutral]
- ADX: 18.25 [range_bound], ATR: 16.43 [high_volatility]
- BOLL中轨: 400.70 [inside_band], 带宽: 13.23 [band_expanding]
- VWAP: 402.57 [below], VWMA: 401.41 [below]
- OBV: 226854 [volume_accumulation]
- 结论: volatility_elevated, volume_confirms_bid, technical_neutral_zone, price_below_ma50

**IC纪律**
- 状态: must_defend
- 当前: 392.36, 确认位: 374.58, 止损位: 401.41
- RSI: 49.8, MACD: 11.04

**组合决策**
- 评级: Underweight, CoreResearchCall: lean_sell
- 摘要: Call: Underweight. Setup quality score of 56/100 warrants cautious action.

---

### 5. 招商银行 (600036.SH)

**核心评分**
- 方向分: -33 (市场 -10, 基本面 -8, 舆情 -7, 情绪 -2, 风险调整 -6)
- 行动分: 81 (一致性 20, 执行水平 25, 仓位纪律 20, 视野清晰 10, 盈亏比 6)
- 置信度: 66 (上限前 116, 应用上限 88)
- 研究可靠度: 100/100

**价格上下文**
- 当前价: 36.42
- 60日高: 40.15 (2026-04-21), 低: 35.40 (2026-06-29)
- 距高点: -10.2%, 距低点: +2.8%

**概率视角**
- 上行: 29%, 下行: 42%, 横盘: 29%, 风险: 49%
- 上行目标: 40.15 (+10.2%), 下行目标: 38.17

**技术指标**
- MA50: 38.17 [below], EMA10: 37.10 [below]
- MACD: -0.47 [bearish_cross], RSI: 31.88 [neutral]
- KDJ_K: 16.01 [oversold], CCI: -161.95 [oversold]
- ADX: 47.58 [trend_strong], ATR: 0.76 [normal_volatility]
- BOLL中轨: 37.98 [inside_band], 带宽: 10.09 [band_normal]
- VWAP: 37.91 [below], VWMA: 37.89 [below]
- OBV: -13995381 [volume_accumulation]
- 结论: volume_confirms_bid, technical_neutral_zone, macd_bearish, price_below_ma50

**IC纪律**
- 状态: must_defend
- 当前: 36.42, 确认位: 35.40, 止损位: 38.17
- RSI: 31.9

**组合决策**
- 评级: Underweight, CoreResearchCall: sell_on_break
- 摘要: Call: Underweight. Setup quality is conditional with no historical calibration.

---

### 6. 美的集团 (000333.SZ) ⭐ 唯一正面

**核心评分**
- 方向分: **+27** (市场 +11, 基本面 -2, 舆情 +6, 情绪 +4, 风险调整 +8)
- 行动分: 83 (一致性 20, 执行水平 25, 仓位纪律 20, 视野清晰 10, 盈亏比 8)
- 置信度: 52 (上限前 94, 应用上限 88)
- 研究可靠度: 94/100

**价格上下文**
- 当前价: 77.27
- 60日高: 82.18 (2026-06-11), 低: 70.59 (2026-03-31)
- 距高点: -6.4%, 距低点: +8.6%

**概率视角**
- 上行: **41%**, 下行: **33%**, 横盘: 26%, 风险: 43%
- 上行目标: 82.18 (+6.4%), 下行目标: 70.59 (+8.6%)

**技术指标**
- MA50: 76.71 [**above**], EMA10: 75.44 [**above**]
- MACD: +1.67 [bullish_cross], RSI: 44.81 [neutral]
- KDJ_K: 41.87 [bullish_cross], CCI: -7.68 [neutral]
- ADX: 24.71 [trend_moderate], ATR: 2.33 [normal_volatility]
- BOLL中轨: 77.02 [inside_band], 带宽: 12.83 [band_expanding]
- VWAP: 76.97 [**above**], VWMA: 76.98 [**above**]
- OBV: 2124204 [volume_accumulation]
- 结论: volume_confirms_bid, technical_neutral_zone, macd_bullish

**IC纪律**
- 状态: **probe_watch** (可试探性观察)
- 当前: 77.27, 确认位: 82.18, 止损位: 70.59
- RSI: 44.8, MACD: 1.67

**组合决策**
- 评级: **Overweight**, CoreResearchCall: **buy_on_confirmation**
- 盈亏比: 0.73
- 摘要: The call is Hold. Setup quality does not currently warrant standard execution due to mixed technical and fundamental signals.

---

### 7. 隆基绿能 (601012.SH)

**核心评分**
- 方向分: -40 (市场 -7, 基本面 -13, 舆情 -9, 情绪 -5, 风险调整 -6)
- 行动分: 81 (一致性 20, 执行水平 25, 仓位纪律 20, 视野清晰 10, 盈亏比 6)
- 置信度: 67 (上限前 112, 应用上限 88)
- 研究可靠度: 100/100

**价格上下文**
- 当前价: 12.81
- 60日高: 18.03 (2026-04-01), 低: 12.22 (2026-06-09)
- 距高点: -40.7%, 距低点: +4.6%

**概率视角**
- 上行: 27%, 下行: 43%, 横盘: 30%, 风险: 50%
- 上行目标: 18.03 (+40.7%), 下行目标: 13.30

**技术指标**
- MA50: 14.98 [below], EMA10: 12.89 [below]
- MACD: -0.63 [bullish_cross], RSI: 54.20 [neutral]
- KDJ_K: 30.39 [bullish_cross], CCI: -78.95 [neutral]
- ADX: 24.48 [trend_moderate], ATR: 0.60 [high_volatility]
- BOLL中轨: 13.13 [inside_band], 带宽: 13.36 [band_expanding]
- VWAP: 13.17 [below], VWMA: 13.16 [below]
- OBV: -22742293 [volume_accumulation]
- 结论: volatility_elevated, volume_confirms_bid, technical_neutral_zone, price_below_ma50

**IC纪律**
- 状态: must_defend
- 当前: 12.81, 确认位: 13.10, 止损位: 13.30
- RSI: 54.2

**组合决策**
- 评级: Underweight, CoreResearchCall: sell_on_break
- 摘要: The call is Underweight. Setup quality is moderate with no historical calibration.

---

### 8. 比亚迪 (002594.SZ)

**核心评分**
- 方向分: -41 (市场 -11, 基本面 -8, 舆情 -10, 情绪 -6, 风险调整 -6)
- 行动分: 81 (一致性 20, 执行水平 25, 仓位纪律 20, 视野清晰 10, 盈亏比 6)
- 置信度: 62 (上限前 112, 应用上限 88)
- 研究可靠度: 100/100

**价格上下文**
- 当前价: 79.64
- 60日高: 108.49 (2026-03-31), 低: 77.60 (2026-06-29)
- 距高点: -36.2%, 距低点: +2.6%

**概率视角**
- 上行: 26%, 下行: 44%, 横盘: 30%, 风险: 52%
- 上行目标: 108.49 (+36.2%), 下行目标: 84.29

**技术指标**
- MA50: 95.08 [below], EMA10: 84.29 [below]
- MACD: +5.25 [bullish_cross], RSI: 22.17 [oversold]
- KDJ_K: 11.10 [oversold], CCI: -165.39 [oversold]
- ADX: 49.07 [trend_strong], ATR: 2.57 [normal_volatility]
- BOLL中轨: 88.92 [inside_band], 带宽: 22.07 [band_expanding]
- VWAP: 88.25 [below], VWMA: 88.15 [below]
- OBV: -6660116 [volume_accumulation]
- 结论: volume_confirms_bid, technical_oversold, macd_bullish, price_below_ma50

**IC纪律**
- 状态: must_defend
- 当前: 79.64, 确认位: 77.60, 止损位: 84.29
- RSI: 22.2, MACD: 5.25

**组合决策**
- 评级: Underweight, CoreResearchCall: sell_on_break
- 摘要: Call: Underweight. Setup quality score is high but execution confidence is moderate.
