# 紧凑线格式实施记录

状态：current/series/stream、进程列表与进程 current/series/stream 默认使用紧凑数据，JSONL 导出同样使用紧凑批次；旧对象输出分支已移除。`wire=compact` 仍可显式声明，但省略时结果相同。CLI、TSV 与 Prometheus 不受此切换影响。OpenSpec 7.11–7.13 尚未完成全部验收。

API 主版本继续为 1。线格式使用独立标识 `procface-compact-v1`，旧对象协议不保留兼容分支。CLI、TSV API、Prometheus 和前端下载仍使用既有可读格式。

## 批次与目录

批次固定为 `[sequence, uptime, timestamp_unix, group_id, complete, diagnostics, dictionary_delta, samples, processes]`。

样本固定为 `[metric_id, entity_id, value, status_id]`。目录条目为：

- 指标：`[id, name, unit, kind]`；必须由显式发布目录指定编号，不能按采集顺序分配。
- 实体：`[id, name]`；由 daemon session 分配，编号递增、不复用。
- 分组：索引 0–2 对应 `system/process/trace`。
- 状态：索引 0–6 对应 `ok/unsupported/permission_denied/parse_error/exited/stale/discontinuity`。

编码核心目前为每个批次携带其引用的字典子集（定义允许重复），放在样本前。这样窗口中任意起点都可以解码。流端后续可省略已发送定义，但必须保留首次定义及重连上下文；优化必须以实际体积测量为依据。

编码批次持有实体定义的强引用，目录持有弱引用。只有缓存、队列和导出均释放批次后，目录才可回收条目；重新遇到已回收的名称也分配新编号。接入存储时必须将编码批次和这些引用纳入字节预算。浏览器持久化保存展开数据，重连时从带目录的 series 响应恢复。

## 进程记录

固定位置：`[entity_id, pid, starttime_ticks, name, state, uid, rss_bytes, threads, cpu_seconds, cpu_percent, sample_mask, cpu_status_id]`。

`sample_mask` 七个位依次对应 PID、名称、状态、RSS、线程数、CPU 秒数、CPU 百分比样本。只有原始样本确实存在、值匹配、状态可保留时才合并；前端按位恢复可读样本，避免过滤响应凭空产生未请求的样本。CPU 百分比的状态单独保存，首次读取的 `null/stale` 不得变成 `null/ok`。其他异常、退出实体和 `virtual_bytes` 继续保留在样本数组中。

## 验证与接入剩余项

`cargo test --locked --offline` 当前通过 16 项测试，包含紧凑编码专项测试：

- 独立解码器验证进程去重、筛选、异常状态、结构化值及 u64 最大值往返；样例封装体积小于原始对象。
- 采集顺序变化不改变目录编号；最后一个批次引用释放后可回收，编号不复用。
- 重复编号、未知指标和单位/类型冲突必须报错，不能静默解释。

固定指标发布目录已加入 `src/metric-catalog.tsv`，现有编号不得改变或重用，增加指标时显式追加新的编号。静态编号限定在 1 至 2^32−1；`trace.thread.TID.stat` 使用预留区间中的 `2^32 + TID`，覆盖非零 u32 TID，数值仍在 JavaScript 安全整数范围内。线程名称和 Trace 文本保留既有语义，目录中不得按出现顺序分配线程指标编号。动态定义随对应批次提供。

已增加解析器目录覆盖测试，以及模拟 procfs 的实际 system/process/extended Trace 编码与还原测试；诊断构建额外覆盖敏感文件的缺失状态。

默认协议切换后已通过 `test_e2e.py`、`test_stream.py`、`test_trace.py`；测试通过独立的 `tests/compact_wire.py` 解码器检查数组形状、目录引用和原有语义。`test_compact.py` 验证默认数组、逐批独立解码、动态网卡、进程筛选、分页元数据及 JSONL。`tools/test_frontend_compact.cjs` 在真实浏览器中验证 IndexedDB 保存、进程样本恢复、状态及 u64/结构化 Trace；已修复进程身份混入函数导致无法保存历史的问题。

仍需严格拒绝缺少或不兼容 wire schema 的建链声明及旧 daemon、未知字典的有界恢复与缺口、导出边界和真实负载体积测量，以及四架构运行验证和正式发布产物校验。上述工作不能由已有回归通过替代。
