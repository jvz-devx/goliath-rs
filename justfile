test:
    cargo test --workspace

# Default: 5 policies * 3 settings * 66,667 seeds = 1,000,005 scenario-seeds.
sweep seeds="66667":
    cargo run --release -p goliath-sim -- sweep {{seeds}}

replay policy seed setting="0":
    cargo run --release -p goliath-sim -- replay {{policy}} {{seed}} {{setting}}

# Partitioned old owner, including a control that ignores its local deadline.
leases seeds="20000" output="docs/experiments/data/E16/leases.csv":
    cargo run --release -p goliath-sim -- leases {{seeds}} {{output}}

# Long, real-process durability and latency experiment. Never part of `test`.
store-torture kills="1000":
    cargo build --release -p goliath-store --features fault-injection
    python3 -B scripts/e5-store.py --kills {{kills}}
