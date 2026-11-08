"""D9's fixed tile geometry, cohort sizes and pre-run witness plan."""
import math
import random

TILES={'a':(0,0),'b':(-1,0),'c':(-1,-1),'d':(0,-1)}
CROSS_PATH='-12,12;12,12;12,-12;-12,-12;12,12;-12,12;-12,-12;12,-12'


def plan(count,anchor,seed=18):
    crossing=count//10;interior=count-crossing;groups=[];names=[]
    for index,(label,tile) in enumerate(TILES.items()):
        n=interior//4+int(index<interior%4);center=[coordinate*2048+1024 for coordinate in tile];spread=[center[0]-64,center[1]-64,center[0]+64,center[1]+64]
        prefix='E17Bot'+label.upper();radius=min(128/math.ceil(math.sqrt(n))*.2,8)
        starts={prefix+str(i):anchor(i,n,12345,spread) for i in range(n)}
        group=dict(label=label,backend=label,count=n,prefix=prefix,spread=spread,seed=12345,center=center,starts=starts,patrol_radius=radius,host='d9-'+label+'.test',kind='interior')
        groups.append(group);names.extend(starts)
    groups.append(dict(label='cross',backend='a',count=crossing,prefix='E17BotX',path=CROSS_PATH,host='d9-cross.test',kind='crossing'))
    rng=random.Random(seed);witnesses=[rng.choice(list(group['starts'])) for group in groups[:4]]
    witnesses+=rng.sample([name for name in names if name not in witnesses],2)
    witnesses+=['E17BotX'+str(index) for index in rng.sample(range(crossing),2)]
    return dict(layout='D9',edge=2048,world_seed=12345,players=count,interior_players=interior,crossing_players=crossing,
        pigs_per_tile=64,total_pigs=256,witness_seed=seed,witnesses=witnesses,groups=groups,
        minimum_interior_square_wall_distance=960,capacity_claim=False)
