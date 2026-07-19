/**
 * healthServer.ts
 *
 * Lightweight HTTP server that exposes oracle health information.
 *
 * Endpoints
 * ─────────────────────────────────────────────────────────────────────────────
 * GET /health/oracle
 *   Returns a JSON snapshot of the oracle's current metrics.
 *   HTTP 200 when healthy, HTTP 503 when the oracle is considered degraded
 *   (no successful run within 2 × maxAgeSecs).
 *
 * GET /health/live
 *   Simple liveness probe — always returns HTTP 200 {"status":"ok"}.
 *
 * Usage
 * ─────────────────────────────────────────────────────────────────────────────
 * import { createHealthServer } from "./healthServer";
 * const server = createHealthServer(oracle, { port: 8080, maxAgeSecs: 3600 });
 * server.start();
 * // …later…
 * server.stop();
 */

import * as http from "http";
import { RateOracle, OracleMetrics } from "./rateOracle";

export interface HealthServerOptions {
  port?: number;
  /** Used to decide whether the oracle is degraded (last run > 2× this) */
  maxAgeSecs: number;
}

export interface OracleHealthResponse {
  status: "healthy" | "degraded";
  lastSuccessfulRunIso: string | null;
  lastSuccessfulRunAgeMs: number | null;
  totalRuns: number;
  failedRuns: number;
  providerErrors: Record<string, number>;
  pairs: Record<
    string,
    {
      status: "ok" | "error";
      lastSuccessIso: string | null;
      lastError: string | null;
      errorCount: number;
      provider: string | null;
    }
  >;
}

export class HealthServer {
  private server: http.Server;
  private readonly oracle: RateOracle;
  private readonly options: Required<HealthServerOptions>;

  constructor(oracle: RateOracle, options: HealthServerOptions) {
    this.oracle = oracle;
    this.options = { port: 8080, ...options };
    this.server = http.createServer(this.handleRequest.bind(this));
  }

  start(): void {
    this.server.listen(this.options.port, () => {
      console.info(
        `[HealthServer] listening on port ${this.options.port}`
      );
    });
  }

  stop(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.server.close((err) => (err ? reject(err) : resolve()));
    });
  }

  // ── request handler ────────────────────────────────────────────────────────

  private handleRequest(
    req: http.IncomingMessage,
    res: http.ServerResponse
  ): void {
    const url = req.url ?? "/";

    if (url === "/health/live") {
      this.respondJson(res, 200, { status: "ok" });
      return;
    }

    if (url === "/health/oracle") {
      const { body, statusCode } = this.buildOracleHealth();
      this.respondJson(res, statusCode, body);
      return;
    }

    this.respondJson(res, 404, { error: "not found" });
  }

  private buildOracleHealth(): {
    body: OracleHealthResponse;
    statusCode: number;
  } {
    const m: OracleMetrics = this.oracle.metrics;
    const nowMs = Date.now();

    const lastRunAgeMs =
      m.lastSuccessfulRunMs !== null ? nowMs - m.lastSuccessfulRunMs : null;

    const degradedThresholdMs = this.options.maxAgeSecs * 2 * 1_000;
    const isDegraded =
      m.lastSuccessfulRunMs === null ||
      (lastRunAgeMs !== null && lastRunAgeMs > degradedThresholdMs);

    const pairs: OracleHealthResponse["pairs"] = {};
    for (const [key, ps] of Object.entries(m.pairs)) {
      pairs[key] = {
        status: ps.lastError === null ? "ok" : "error",
        lastSuccessIso:
          ps.lastSuccessTs !== null
            ? new Date(ps.lastSuccessTs).toISOString()
            : null,
        lastError: ps.lastError,
        errorCount: ps.errorCount,
        provider: ps.provider,
      };
    }

    const body: OracleHealthResponse = {
      status: isDegraded ? "degraded" : "healthy",
      lastSuccessfulRunIso:
        m.lastSuccessfulRunMs !== null
          ? new Date(m.lastSuccessfulRunMs).toISOString()
          : null,
      lastSuccessfulRunAgeMs: lastRunAgeMs,
      totalRuns: m.totalRuns,
      failedRuns: m.failedRuns,
      providerErrors: { ...m.providerErrors },
      pairs,
    };

    return { body, statusCode: isDegraded ? 503 : 200 };
  }

  private respondJson(
    res: http.ServerResponse,
    statusCode: number,
    body: unknown
  ): void {
    const json = JSON.stringify(body, null, 2);
    res.writeHead(statusCode, {
      "Content-Type": "application/json",
      "Content-Length": Buffer.byteLength(json),
    });
    res.end(json);
  }
}

/** Convenience factory */
export function createHealthServer(
  oracle: RateOracle,
  options: HealthServerOptions
): HealthServer {
  return new HealthServer(oracle, options);
}
