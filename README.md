# indexer

Read-only explorer indexer for Pinaivu. Serves the public API behind
[explorer.pinaivu.com](https://explorer.pinaivu.com): per-request
provenance (routing receipt + proofs), node activity, and a recent-
receipts feed. It is off-chain, and read-only with respect to the
coordinator's data — it reads the coordinator's Postgres and live peer
registry, and archives old receipts to Walrus.

This service never touches the coordinator's signing key, the Sui
contracts, or the node mesh directly. See the [decentralization &
verifiability
model](https://docs.pinaivu.com/architecture/decentralization) for
where receipts and proofs come from.

## Endpoints

| Endpoint | Purpose |
|---|---|
| `GET /health` | Liveness |
| `GET /api/r/{request_id}` | Full receipt: proofs, payouts, on-chain verification status |
| `GET /api/nodes` | Current node activity snapshot |
| `GET /api/nodes/{peer_id}` | One node's recent receipts |
| `GET /api/recent` | Paginated recent receipts |

A cron job periodically batches receipts older than
`ARCHIVE_AFTER_MINUTES` that haven't been archived yet, uploads them to
Walrus, and records the blob id back in Postgres.

## Run

```bash
cp .env.example .env   # fill in DATABASE_URL, WALRUS_PUBLISHER_URL, COORDINATOR_URL
cargo run
```

See `.env.example` for the full set of environment variables, including
`INSECURE_COORDINATOR` (the coordinator presents a self-signed attested
cert) and `NODE_ONLINE_TTL_SECS`.

## Tests

```bash
cargo test
```
