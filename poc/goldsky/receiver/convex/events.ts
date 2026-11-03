import { internalMutation, query } from "./_generated/server";
import { v } from "convex/values";

function tryParse(s: string): unknown | undefined {
  try {
    return JSON.parse(s);
  } catch {
    return undefined;
  }
}

function str(x: unknown): string {
  return typeof x === "string" ? x : x == null ? "" : JSON.stringify(x);
}

// Stores one webhook batch. Goldsky delivers at least once, so `eventId` is the
// dedupe key: a row we already hold is counted and skipped. Everything for one
// batch is written in a single mutation, so a crash leaves no half-written batch.
export const ingest = internalMutation({
  args: { rows: v.array(v.any()) },
  handler: async (ctx, { rows }) => {
    let inserted = 0;
    let duplicates = 0;
    let firstLedger: number | undefined;
    let lastLedger: number | undefined;

    for (const row of rows) {
      const eventId = str(row.id);
      if (!eventId) continue;
      const seen = await ctx.db
        .query("events")
        .withIndex("by_event_id", (q) => q.eq("eventId", eventId))
        .unique();
      const ledger = Number(row.ledger_sequence);
      firstLedger = firstLedger === undefined ? ledger : Math.min(firstLedger, ledger);
      lastLedger = lastLedger === undefined ? ledger : Math.max(lastLedger, ledger);
      if (seen) {
        duplicates++;
        continue;
      }
      const topicsRaw = str(row.topics);
      const dataRaw = str(row.data);
      await ctx.db.insert("events", {
        eventId,
        contractId: str(row.contract_id),
        ledger,
        closedAt: str(row.ledger_closed_at),
        txHash: str(row.transaction_hash),
        txSuccessful: Boolean(row.transaction_successful),
        inSuccessfulCall: Boolean(row.in_successful_contract_call),
        topicsRaw,
        dataRaw,
        topics: tryParse(topicsRaw),
        data: tryParse(dataRaw),
        raw: row,
      });
      inserted++;
    }

    await ctx.db.insert("deliveries", {
      receivedAt: Date.now(),
      rows: rows.length,
      inserted,
      duplicates,
      firstLedger,
      lastLedger,
    });
    return { inserted, duplicates };
  },
});

// Latest events, optionally for one contract. Use from the dashboard to inspect
// what arrived and how topics/data are encoded.
export const list = query({
  args: { contractId: v.optional(v.string()), limit: v.optional(v.number()) },
  handler: async (ctx, { contractId, limit }) => {
    const n = Math.min(limit ?? 50, 500);
    if (contractId) {
      return await ctx.db
        .query("events")
        .withIndex("by_contract_ledger", (q) => q.eq("contractId", contractId))
        .order("desc")
        .take(n);
    }
    return await ctx.db.query("events").order("desc").take(n);
  },
});

// Counts per contract and the delivery log, for a quick health check.
export const stats = query({
  args: {},
  handler: async (ctx) => {
    const events = await ctx.db.query("events").collect();
    const byContract: Record<string, { count: number; firstLedger: number; lastLedger: number }> = {};
    for (const e of events) {
      const c = (byContract[e.contractId] ??= { count: 0, firstLedger: e.ledger, lastLedger: e.ledger });
      c.count++;
      c.firstLedger = Math.min(c.firstLedger, e.ledger);
      c.lastLedger = Math.max(c.lastLedger, e.ledger);
    }
    const deliveries = await ctx.db.query("deliveries").order("desc").take(20);
    return { total: events.length, byContract, recentDeliveries: deliveries };
  },
});
