"""Owns equal nine-driver witness separation without changing D9 player routes."""
import copy


def split_witnesses(layout,witnesses,direct):
    result = copy.deepcopy(layout)
    extra = []
    for group in result['groups']:
        group['indices'] = list(range(group['count']))
        group['route_bots'] = group['count']
        if group['kind'] != 'interior':
            continue
        indices = sorted(row['index'] for row in witnesses if row['tile'] == group['backend'])
        if len(indices) != 2 or any(index >= group['count'] for index in indices):
            raise ValueError('two valid witnesses per original tile cohort required')
        selected = copy.deepcopy(group)
        selected.update(label='witness-'+group['label'],count=2,indices=indices,witness_group=True)
        if direct:
            selected['direct_tile'] = group['backend']
        group['count'] -= 2
        group['indices'] = [index for index in group['indices'] if index not in indices]
        extra.append(selected)
    result['groups'].extend(extra)
    if len(extra) != 4 or sum(group['count'] for group in result['groups']) != layout['players']:
        raise ValueError('witness separation changed total population')
    result.update(witnesses=[row['name'] for row in witnesses],witness_seed=25,
        native_handshake_hosts=True,driver_processes=9,direct_witnesses=8 if direct else 0,
        proxy_players=layout['players']-(8 if direct else 0),capacity_claim=False)
    return result
