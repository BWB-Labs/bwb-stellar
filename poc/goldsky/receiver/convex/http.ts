import { httpRouter } from "convex/server";
import { httpAction } from "./_generated/server";
import { internal } from "./_generated/api";

const http = httpRouter();

// Goldsky Turbo webhook sink. The pipeline POSTs a JSON array of rows (one per
// event) and adds the header stored in its `httpauth` secret. We expect that
// header to be `Authorization: Bearer <GOLDSKY_WEBHOOK_TOKEN>`.
//
// Status codes matter: Goldsky retries 5xx/408/429 forever and FAILS the pipeline
// on any other 4xx. So: wrong token -> 401 (misconfiguration, stop the pipeline);
// transient failure in our mutation -> 500 (retry the same batch).
http.route({
  path: "/goldsky",
  method: "POST",
  handler: httpAction(async (ctx, req) => {
    const expected = process.env.GOLDSKY_WEBHOOK_TOKEN;
    if (!expected) {
      return new Response("GOLDSKY_WEBHOOK_TOKEN is not set on this deployment", { status: 500 });
    }
    if (req.headers.get("authorization") !== `Bearer ${expected}`) {
      return new Response("unauthorized", { status: 401 });
    }

    let body: unknown;
    try {
      body = await req.json();
    } catch {
      return new Response("body is not JSON", { status: 400 });
    }
    const rows = Array.isArray(body) ? body : [body];

    try {
      const result = await ctx.runMutation(internal.events.ingest, { rows });
      return Response.json({ received: rows.length, ...result });
    } catch (err) {
      console.error("ingest failed, asking Goldsky to retry", err);
      return new Response("ingest failed", { status: 500 });
    }
  }),
});

// Liveness check: GET https://<deployment>.convex.site/goldsky
http.route({
  path: "/goldsky",
  method: "GET",
  handler: httpAction(async () => Response.json({ ok: true, service: "bwb-goldsky-receiver" })),
});

export default http;
