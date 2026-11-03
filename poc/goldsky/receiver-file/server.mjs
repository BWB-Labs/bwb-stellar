// POC receiver with no database: every row Goldsky POSTs is appended as one JSON
// line to events.jsonl, and every request is logged to deliveries.log.
//
//   node server.mjs                      # listens on :8787, accepts any request
//   GOLDSKY_WEBHOOK_TOKEN=xyz node server.mjs   # requires "Authorization: Bearer xyz"
//
// Goldsky retries 5xx forever and fails the pipeline on other 4xx, so a wrong
// token answers 401 on purpose and a write error answers 500.
import { createServer } from "node:http";
import { appendFileSync, existsSync, readFileSync } from "node:fs";

const PORT = Number(process.env.PORT ?? 8787);
const TOKEN = process.env.GOLDSKY_WEBHOOK_TOKEN;
const EVENTS = new URL("./events.jsonl", import.meta.url).pathname;
const LOG = new URL("./deliveries.log", import.meta.url).pathname;

// Dedupe across restarts: Goldsky delivers at least once.
const seen = new Set(
  existsSync(EVENTS)
    ? readFileSync(EVENTS, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l).id)
    : [],
);

createServer((req, res) => {
  if (req.method === "GET") {
    res.writeHead(200, { "content-type": "application/json" });
    return res.end(JSON.stringify({ ok: true, events: seen.size }));
  }
  if (req.method !== "POST") return res.writeHead(405).end();
  if (TOKEN && req.headers.authorization !== `Bearer ${TOKEN}`) return res.writeHead(401).end("unauthorized");

  let body = "";
  req.on("data", (c) => (body += c));
  req.on("end", () => {
    let rows;
    try {
      const parsed = JSON.parse(body);
      rows = Array.isArray(parsed) ? parsed : [parsed];
    } catch {
      return res.writeHead(400).end("body is not JSON");
    }
    let inserted = 0, duplicates = 0;
    try {
      for (const row of rows) {
        if (seen.has(row.id)) { duplicates++; continue; }
        appendFileSync(EVENTS, JSON.stringify(row) + "\n");
        seen.add(row.id);
        inserted++;
      }
      const ledgers = rows.map((r) => Number(r.ledger_sequence)).filter(Number.isFinite);
      const line = `${new Date().toISOString()} rows=${rows.length} inserted=${inserted} duplicates=${duplicates} ledgers=${Math.min(...ledgers)}..${Math.max(...ledgers)}`;
      appendFileSync(LOG, line + "\n");
      console.log(line);
    } catch (err) {
      console.error("write failed, asking Goldsky to retry", err);
      return res.writeHead(500).end("write failed");
    }
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ received: rows.length, inserted, duplicates }));
  });
}).listen(PORT, () => console.log(`receiver on http://localhost:${PORT}  -> ${EVENTS}`));
