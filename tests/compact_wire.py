"""独立参考解码器：测试默认 API 线格式，并复用既有语义断言。"""
GROUPS = ['system', 'process', 'trace']
STATUSES = ['ok', 'unsupported', 'permission_denied', 'parse_error', 'exited', 'stale', 'discontinuity']
PROCESS = ['process.pid', 'process.name', 'process.state', 'process.rss_bytes', 'process.threads', 'process.cpu_seconds', 'process.cpu_usage']


def batch(raw, session):
    assert isinstance(raw, list) and len(raw) == 9
    sequence, uptime, timestamp, group, complete, diagnostics, dictionary, samples, processes = raw
    metrics = {m[0]: m for m in dictionary['metrics']}
    entities = dict(dictionary['entities'])
    out = dict(schema_version=1, session_id=session, sequence=sequence, uptime_s=uptime,
               timestamp_unix=timestamp, group=GROUPS[group], complete=complete,
               diagnostics=diagnostics, samples=[], processes=[])
    def sample(m, entity, value, status):
        out['samples'].append(dict(schema_version=1, session_id=session, sequence=sequence,
            uptime_s=uptime, timestamp_unix=timestamp, metric=m[1], entity=entities[entity],
            value=value, unit=m[2], kind=m[3], status=STATUSES[status]))
    for m, entity, value, status in samples:
        sample(metrics[m], entity, value, status)
    for p in processes:
        assert len(p) == 12 and p[0] in entities
        out['processes'].append(dict(identity=dict(pid=p[1], starttime_ticks=p[2]),
            name=p[3], state=p[4], uid=p[5], rss_bytes=p[6], threads=p[7], cpu_seconds=p[8], cpu_percent=p[9]))
        for bit, column in enumerate([1,3,4,6,7,8,9]):
            if p[10] & (1 << bit):
                m = next(m for m in metrics.values() if m[1] == PROCESS[bit])
                sample(m, p[0], p[column], p[11] if bit == 6 else 0)
    return out


def decode(value):
    assert value['wire_schema'] == 'procface-compact-v1'
    assert value['schema_version'] == 1
    if 'batch' in value:
        return batch(value['batch'], value['session_id'])
    return {**value, 'batches': [batch(b, value['session_id']) for b in value['batches']]}
