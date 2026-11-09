"""Owns Linux BOOTTIME lease evidence without inventing Darwin clock histograms."""
import gzip
import json
from pathlib import Path

PERIOD = 250_000_000


def authority_point(reply):
    if reply.get('authorized') is not True or reply.get('leased') is not True or reply.get('clock') is not None:
        raise RuntimeError('Linux BOOTTIME authority telemetry missing or unauthorized')
    return dict(schema='E28-boottime-authority-v1', authority=reply,
        clock_histogram=None, unavailable='Darwin raw-clock histogram is not exposed by Linux')


def trace_result(events, begin_wall_ns, end_wall_ns):
    starts = [row for row in events if row['event'] == 'trace-start']
    tickets = [row for row in events if row['event'] == 'renew-ticket']
    installed = [row for row in events if row['event'] == 'renew-installed']
    sends = [row for row in events if row['event'] == 'renew-send']
    replies = [row for row in events if row['event'] == 'renew-reply']
    woke = [row for row in events if row['event'] == 'renew-woke']
    closures = [row for row in events if row['event'] in {'authority-stop', 'clock-anomaly', 'renew-observe-refused'}]
    ordered = all(b['data']['counter'] > a['data']['counter'] for a, b in zip(tickets, tickets[1:]))
    grants = all(row['data']['counter'] == row['data']['grant_counter'] for row in installed)
    grants = grants and all(b['data']['grant_counter'] > a['data']['grant_counter'] for a, b in zip(installed, installed[1:]))
    anchored = []
    wrong = []
    for row in woke:
        prior = next((ticket for ticket in reversed(tickets) if ticket['at_ns'] < row['data']['sleep_start_ns']), None)
        # The first wait is anchored to the claim, not to a previous renewal.
        if prior:
            delta = row['data']['planned_wake_ns'] - prior['data']['sent_ns']
            anchored.append(delta)
            if delta != PERIOD or row['data']['period_ns'] != PERIOD:
                wrong.append(row)
    deadlines = all(row['data']['deadline_ns'] - row['data']['sent_ns'] == 900_000_000 for row in installed)
    call_budget = all(row['data']['call_deadline_ns'] == row['data']['deadline_ns'] - 50_000_000 for row in tickets)
    active = [row for row in installed if begin_wall_ns <= row['wall_ns'] <= end_wall_ns]
    all_installs = sorted(installed, key=lambda row:row['wall_ns'])
    coverage = bool(active and all_installs[0]['wall_ns'] <= begin_wall_ns
        and end_wall_ns - all_installs[-1]['wall_ns'] < 1_000_000_000)
    gaps = [b['data']['sent_ns'] - a['data']['sent_ns'] for a, b in zip(tickets, tickets[1:])]
    loaded_tickets={row['data']['counter'] for row in tickets if begin_wall_ns <= row['wall_ns'] <= end_wall_ns}
    sent_counters={row['data']['counter'] for row in sends}
    replied_counters={row['data']['counter'] for row in replies}
    installed_counters={row['data']['counter'] for row in installed}
    successful={row['data']['counter'] for row in replies if row['data'].get('result','').startswith('Ok(Leased(')}
    errors=[row['data'] for row in replies if row['data'].get('result','').startswith('Err(')]
    failed_counters={row['counter'] for row in errors}
    complete=bool(loaded_tickets and loaded_tickets<=sent_counters & replied_counters
        and loaded_tickets<=successful | failed_counters
        and (loaded_tickets & successful)<=installed_counters)
    return dict(valid=bool(len(starts) == 1 and ordered and grants and anchored and not wrong and deadlines
            and call_budget and coverage and complete and not closures),
        records=len(events), trace_identity=starts[0]['data'] if len(starts) == 1 else None,
        renewal_tickets=len(tickets), renewal_installs=len(installed), loaded_renewal_installs=len(active),
        first_counter=installed[0]['data']['grant_counter'] if installed else None,
        last_counter=installed[-1]['data']['grant_counter'] if installed else None,
        counters_increasing=ordered and grants, send_anchored_250ms=bool(anchored and not wrong),
        checked_wakes=len(anchored), cadence_errors=wrong, original_send_deadlines=deadlines,
        loaded_call_ledger_complete=complete,
        all_loaded_calls_installed=loaded_tickets<=installed_counters,
        non_grant_replies=errors,
        non_grant_reply_interpretation='recorded Err replies may retry within the unchanged confirmed deadline; every successful grant still requires its install record',
        keeper_deadline_minus_50ms_call_budget=call_budget, loaded_interval_covered=coverage,
        send_gap_ms=dict(min=min(gaps)/1e6, max=max(gaps)/1e6) if gaps else None,
        authority_or_clock_events=closures, clock_histogram=None,
        clock_policy='Linux BOOTTIME against MONOTONIC Instant brackets, rebased after each accepted check; slack maximum/500 + 1us')


def readback(folder, endpoints):
    first, last = endpoints['first'], endpoints['last']
    if not first or not last or last['mark'] != 'steady-last':
        raise RuntimeError('Linux authority endpoints incomplete')
    tiles = {}
    traces = list((folder/'tiles').glob('*-lease.jsonl*'))
    for label, before in first['tiles'].items():
        after = last['tiles'][label]
        if before['schema'] != 'E28-boottime-authority-v1' or after['authority'] != before['authority']:
            raise RuntimeError('Linux authority changed across the loaded interval')
        index = before['authority']['tile']
        matches = []
        for path in traces:
            from e28_log_integrity import read_trace
            events,integrity=read_trace(path)
            starts = [row for row in events if row['event'] == 'trace-start' and row['data']['tile'] == index]
            if starts and starts[0]['wall_ns'] <= first['wall_ns'] and any(row['wall_ns'] >= last['wall_ns']-1_000_000_000 for row in events):
                matches.append((path, events,integrity))
        if len(matches) != 1:
            raise RuntimeError('exactly one whole-loaded-interval lease trace required for tile '+label)
        path, events,integrity = matches[0]
        tiles[label] = dict(trace_result(events, first['wall_ns'], last['wall_ns']), trace_file=path.name,integrity=integrity)
        tiles[label]['valid']=tiles[label]['valid'] and integrity['complete']
    return dict(kind='E28-boottime-lease-readback', seconds=last['at']-first['at'],
        all_valid=len(tiles) == 4 and all(row['valid'] for row in tiles.values()), tiles=tiles,
        raw_clock_guard_pass=all(not row['authority_or_clock_events'] for row in tiles.values()),
        real_suspend_tested=False, histogram_available=False)
