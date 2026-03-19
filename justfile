# Run the CLI with game mode and data directory
run:
    cargo run -- -g -r ./data

# Export tracker TSV from game memory
export:
    cargo run -- export -o .agent/tracker.tsv

# Export, normalize against iidxapi, and sync charts to D1
update-charts:
    just export
    cd scripts/normalize && npx tsx normalize.ts
    cd web && npm run charts:sync:remote

# Same as update-charts but sync to local D1
update-charts-local:
    just export
    cd scripts/normalize && npx tsx normalize.ts
    cd web && npm run charts:sync:local
