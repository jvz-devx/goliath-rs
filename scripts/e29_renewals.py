"""Owns the soak readback for a store that answers granted renewals before the barrier (E29).

Early renewals leave the batch and request traces and go to `<trace>.renewals.csv`.
Every renewal a tile sent is looked up in both, by request id, so a reader can
never count zero renewals without saying so.
"""
import csv
import gzip
import io
import json
import math
from pathlib import Path
import re

import e28_log_integrity as logs


def raw(path):
    """Read a trace that the collector may have compressed. None when neither form exists."""
    path = Path(path)
    if path.exists():
        return path.read_bytes()
    packed = path.with_suffix(path.suffix+'.gz')
    return gzip.decompress(packed.read_bytes()) if packed.exists() else None


def table(path):
    content = raw(path)
    if content is None:
        return None
    text = content.decode()
    return list(csv.DictReader(io.StringIO(text[:text.rfind('\n')+1])))


def quantile(values, fraction):
    return values[max(0, math.ceil(fraction*len(values))-1)] if values else None


def distribution(values, thresholds):
    values = sorted(values)
    return dict(count=len(values), p50_ms=quantile(values, .5), p99_ms=quantile(values, .99),
        max_ms=values[-1] if values else None, **{f'over_{limit}ms': sum(value > limit for value in values) for limit in thresholds})


def origin_wall_ns(folder):
    """Store rows count from the store's origin; the collector bracketed it with wall time."""
    origin = json.loads(raw(Path(folder)/'store-batches.origin.json'))
    if not re.fullmatch(r'Instant \{ tv_sec: \d+, tv_nsec: \d+ \}', origin['native_instant']):
        raise RuntimeError('unknown store origin format; do not infer alignment')
    return origin['wall_before_ns']


def tile_renewals(folder, start_ns, end_ns):
    """Renewals of the tile processes that ran the loaded interval, from their own lease traces."""
    tiles, sends, stops = {}, {}, []
    for path in sorted((Path(folder)/'tiles').glob('*-lease.jsonl*')):
        events, integrity = logs.read_trace(path)
        start = next(row for row in events if row['event'] == 'trace-start')
        if start['wall_ns'] > start_ns:
            continue  # the recovery restart after the loaded interval
        label = 'abcd'[start['data']['tile']]
        offset = start['wall_ns']-start['at_ns']
        inside = lambda nanoseconds: start_ns <= nanoseconds+offset <= end_ns
        replies = [row['data'] for row in events if row['event'] == 'renew-reply' and inside(row['data']['sent_ns'])]
        durations = [(row['reply_ns']-row['sent_ns'])/1e6 for row in replies]
        for row in events:
            if row['event'] == 'renew-send' and inside(row['data']['sent_ns']):
                sends[row['data']['request_id']] = dict(tile=label, counter=row['data']['counter'],
                    loaded_s=(row['data']['sent_ns']+offset-start_ns)/1e9, result=None)
            if row['event'] in {'authority-stop', 'clock-anomaly', 'renew-observe-refused'}:
                stops.append(dict(tile=label, event=row['event'], cause=row['data'].get('cause'), loaded_s=(row['at_ns']+offset-start_ns)/1e9))
        for row in replies:
            if row['request_id'] in sends:
                sends[row['request_id']]['result'] = 'granted' if row['result'].startswith('Ok(Leased') else row['result']
        tiles[label] = dict(sent=sum(1 for row in sends.values() if row['tile'] == label), replies=len(replies),
            granted=sum(row['result'].startswith('Ok(Leased') for row in replies),
            errors=sum(not row['result'].startswith('Ok(Leased') for row in replies),
            reply_ms=distribution(durations, [200]), trace_complete=integrity['complete'], path=path.name)
    return tiles, sends, stops


def store_memory(folder, zero, span):
    """The store's own memory trace on the loaded clock: how its bounded receipt window filled."""
    rows = table(Path(folder)/'store-batches.memory.csv')
    if not rows:
        return None
    keys = ['accepted', 'receipts', 'receipt_blob_bytes', 'receipt_inline_bytes', 'player_blob_bytes', 'snapshot_len']
    loaded = lambda row: int(row['at_ns'])/1e9-zero
    minutes = []
    for minute in range(0, int(span//60)+1, 5):
        row = min(rows, key=lambda value: abs(loaded(value)-minute*60))
        if abs(loaded(row)-minute*60) < 5:
            minutes.append(dict(minute=minute, **{key: int(row[key]) for key in keys}))
    full = next((int(row['window_full_at_ns'])/1e9-zero for row in rows if int(row['window_full_at_ns']) >= 0), None)
    inside = [row for row in rows if 0 <= loaded(row) <= span]
    return dict(window_full_at_loaded_s=full, receipt_capacity=max(int(row['receipts']) for row in rows),
        every_five_minutes=minutes, receipts_per_second=(int(inside[-1]['accepted'])-int(inside[0]['accepted']))/(loaded(inside[-1])-loaded(inside[0])) if len(inside) > 1 else None,
        scope='receipts held, their blob bytes and the snapshot buffer, from the store memory trace')


def report(folder, start_ns, end_ns, mode):
    """Barriers, renewal replies and the early-renewal accounting for one loaded interval."""
    folder = Path(folder)
    zero = (start_ns-origin_wall_ns(folder))/1e9  # loaded second 0 on the store's clock
    span = (end_ns-start_ns)/1e9
    loaded = lambda nanoseconds: int(nanoseconds)/1e9-zero
    batches = table(folder/'store-batches.csv')
    if batches is None:
        raise RuntimeError('store batch trace missing')
    barriers = [float(row['barrier_ms']) for row in batches
        if float(row['barrier_ms']) > 0 and 0 <= int(row['start_us'])/1e6-zero <= span]
    requests = table(folder/'store-batches.requests.csv')
    journaled = {row['request_id'] for row in requests} if requests is not None else None
    early_rows = table(folder/'store-batches.renewals.csv')
    early = {row['request_id']: row for row in early_rows} if early_rows is not None else None
    tiles, sends, stops = tile_renewals(folder, start_ns, end_ns)
    counts = dict(early=0, journaled=0, unmatched=0)
    unmatched = []
    for request, row in sends.items():
        if early is not None and request in early:
            counts['early'] += 1
        elif journaled is not None and request in journaled:
            counts['journaled'] += 1
        else:
            counts['unmatched'] += 1
            unmatched.append(dict(row, request_id=request))
    inside = [row for row in (early_rows or []) if 0 <= loaded(row['received_ns']) <= span]
    problems = []
    if early is None:
        problems.append('the store wrote no renewals.csv: this store does not answer renewals early, or its trace was lost')
    if requests is None:
        problems.append('store request trace missing')
    if mode == 'default' and not counts['early']:
        problems.append('default mode but no tile renewal was found in renewals.csv')
    if mode == 'barrier' and counts['early']:
        problems.append('barrier mode but renewals were answered early')
    if any(row['result'] == 'granted' for row in unmatched):
        problems.append('a renewal a tile saw granted is in neither store trace')
    if not all(row['trace_complete'] for row in tiles.values()) or len(tiles) != 4:
        problems.append('tile lease traces incomplete')
    largest = max(barriers, default=0)
    return dict(kind='E29-soak-store-renewals', store_renewals=mode, loaded_seconds=span,
        barriers=dict(distribution(barriers, [50, 100, 200, 400]),
            scope='batches with a durable barrier that started inside the loaded interval',
            contains_barrier_over_400ms=largest > 400,
            tested_the_change='yes: the run contains a barrier over 400 ms' if largest > 400 else
                'NO: no barrier over 400 ms in this run, so it did not test renewals against a long stall'),
        tile_renewals=dict(tiles=tiles, sent=len(sends), replies=sum(row['replies'] for row in tiles.values()),
            over_200ms=sum(row['reply_ms']['over_200ms'] for row in tiles.values()),
            max_ms=max((row['reply_ms']['max_ms'] or 0 for row in tiles.values()), default=None)),
        early_renewals=None if early_rows is None else dict(answered_whole_run=len(early_rows), answered_in_loaded_interval=len(inside),
            answer_ms=distribution([(int(row['answered_ns'])-int(row['received_ns']))/1e6 for row in inside], [1, 10, 50]),
            receive_ms=distribution([(int(row['received_ns'])-int(row['accepted_ns']))/1e6 for row in inside], [1, 10, 50]),
            scope='answer is store receipt to answer; receive is connection accept to receipt'),
        accounting=dict(tile_sends=len(sends), answered_early=counts['early'], journaled=counts['journaled'],
            in_neither_trace=counts['unmatched'], in_neither_trace_rows=unmatched[:50],
            rule='every renewal a tile sent in the loaded interval is looked up by request id in renewals.csv and in the request trace'),
        store_memory=store_memory(folder, zero, span),
        authority_events=stops, complete=not problems, problems=problems)
