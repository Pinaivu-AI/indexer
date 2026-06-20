#!/usr/bin/env node
// Decode a Walrus blob written by archive.rs's batch archiver into
// readable JSON. The blob is a CBOR array of receipt_json values; byte
// arrays (hashes, signatures, pubkeys) get hex-encoded for readability.
//
// Usage:
//   node decode-walrus-blob.mjs --blob-id <id> [--request-id <uuid>] [--aggregator <url>]
//
// Examples:
//   node decode-walrus-blob.mjs --blob-id vnG1nnyjbKK6PIiIGThfzQGNW2yZQ1iJV1Y14mPG8HQ
//   node decode-walrus-blob.mjs --blob-id vnG1nnyjbKK6PIiIGThfzQGNW2yZQ1iJV1Y14mPG8HQ --request-id b5dbfe5a-64c7-42dd-a257-d57315314d7d

import cbor from "cbor";

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i++) {
    if (argv[i].startsWith("--")) {
      args[argv[i].slice(2)] = argv[i + 1];
      i++;
    }
  }
  return args;
}

// Byte-array fields get hex-encoded; everything else passes through.
// Heuristic: an array of integers all in [0,255] is treated as bytes.
function hexifyBytes(value) {
  if (Array.isArray(value)) {
    if (value.length > 0 && value.every((v) => typeof v === "number" && v >= 0 && v <= 255 && Number.isInteger(v))) {
      return Buffer.from(value).toString("hex");
    }
    return value.map(hexifyBytes);
  }
  if (value && typeof value === "object") {
    const out = {};
    for (const [k, v] of Object.entries(value)) out[k] = hexifyBytes(v);
    return out;
  }
  return value;
}

function withIsoTimestamp(receipt) {
  if (typeof receipt.timestamp_ms === "number") {
    receipt.timestamp_iso = new Date(receipt.timestamp_ms).toISOString();
  }
  return receipt;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (!args["blob-id"]) {
    console.error("usage: node decode-walrus-blob.mjs --blob-id <id> [--request-id <uuid>] [--aggregator <url>]");
    process.exit(1);
  }

  const aggregator = args.aggregator ?? "https://aggregator.walrus-testnet.walrus.space";
  const url = `${aggregator}/v1/blobs/${args["blob-id"]}`;

  const resp = await fetch(url);
  if (!resp.ok) {
    console.error(`fetch failed: ${resp.status} ${resp.statusText}`);
    process.exit(1);
  }
  const buf = Buffer.from(await resp.arrayBuffer());

  const decoded = cbor.decodeFirstSync(buf);
  const receipts = Array.isArray(decoded) ? decoded : [decoded];

  const pretty = receipts.map((r) => withIsoTimestamp(hexifyBytes(r)));

  if (args["request-id"]) {
    const match = pretty.find((r) => r.request_id === args["request-id"]);
    if (!match) {
      console.error(`request_id ${args["request-id"]} not found in this blob (${receipts.length} receipts present)`);
      process.exit(1);
    }
    console.log(JSON.stringify(match, null, 2));
    return;
  }

  console.error(`# ${receipts.length} receipt(s) in this blob`);
  console.log(JSON.stringify(pretty, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
