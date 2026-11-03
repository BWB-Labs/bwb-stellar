import { defineSchema, defineTable } from "convex/server";
import { v } from "convex/values";

export default defineSchema({
  // One row per Soroban contract event, keyed by the Stellar event id (a sortable TOID).
  events: defineTable({
    eventId: v.string(),
    contractId: v.string(),
    ledger: v.number(),
    closedAt: v.string(),
    txHash: v.string(),
    txSuccessful: v.boolean(),
    inSuccessfulCall: v.boolean(),
    // Goldsky sends `topics` and `data` as strings. We keep the string and, when it
    // parses as JSON, the parsed value, so the POC shows us the exact encoding.
    topicsRaw: v.string(),
    dataRaw: v.string(),
    topics: v.optional(v.any()),
    data: v.optional(v.any()),
    raw: v.any(),
  })
    .index("by_event_id", ["eventId"])
    .index("by_contract_ledger", ["contractId", "ledger"]),

  // One row per webhook request, so we can see batch sizes, duplicates and gaps.
  deliveries: defineTable({
    receivedAt: v.number(),
    rows: v.number(),
    inserted: v.number(),
    duplicates: v.number(),
    firstLedger: v.optional(v.number()),
    lastLedger: v.optional(v.number()),
  }),
});
