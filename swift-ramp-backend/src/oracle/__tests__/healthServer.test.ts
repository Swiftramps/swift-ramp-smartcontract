/**
 * healthServer.test.ts
 *
 * Integration tests for the /health/oracle and /health/live endpoints.
 * A real HTTP server is started on a random port; no mocking framework needed.
 */

import * as http from "http";
import { Keypair, Networks } from "@stellar/stellar-sdk";
import { RateOracle, OracleConfig } from "../rateOracle";
import { createHealthServer } from "../healthServer";

// ── helpers ───────────────────────────────────────────────────────────────────

function buildOracle(overrides?: Partial<OracleConfig>): RateOracle {
  const config: OracleConfig = {
    rpcUrl: "https://fake.rpc",
    networkPassphrase: Networks.TESTNET,
    contractId: "CFAKE0000000000000000000000000000000000000000000000000000",
    adminKeypair: Keypair.random(),
    intervalSecs: 60,
    maxAgeSecs: 300,
    currencyPairs: [["USD", "NGN"]],
    providers: [],
    ...overrides,
  };
  return new RateOracle(config);
}

async function getJson(url: string): Promise<{ status: number; body: unknown }> {
  return new Promise((resolve, reject) => {
    http
      .get(url, (res) => {
        let raw = "";
        res.on("data", (chunk) => (raw += chunk));
        res.on("end", () => {
          try {
            resolve({ status: res.statusCode ?? 0, body: JSON.parse(raw) });
          } catch (e) {
            reject(e);
          }
        });
      })
      .on("error", reject);
  });
}

function getAvailablePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const srv = http.createServer();
    srv.listen(0, () => {
      const addr = srv.address();
      if (!addr || typeof addr === "string") return reject(new Error("bad addr"));
      const port = addr.port;
      srv.close(() => resolve(port));
    });
  });
}

// ── tests ─────────────────────────────────────────────────────────────────────

describe("HealthServer", () => {
  let port: number;

  beforeAll(async () => {
    port = await getAvailablePort();
  });

  it("GET /health/live always returns 200 {status:'ok'}", async () => {
    const oracle = buildOracle();
    const server = createHealthServer(oracle, { port, maxAgeSecs: 300 });
    server.start();

    try {
      const { status, body } = await getJson(
        `http://localhost:${port}/health/live`
      );
      expect(status).toBe(200);
      expect((body as Record<string, unknown>).status).toBe("ok");
    } finally {
      await server.stop();
    }
  });

  it("GET /health/oracle returns 503 when no successful run has occurred", async () => {
    const port2 = await getAvailablePort();
    const oracle = buildOracle();
    const server = createHealthServer(oracle, {
      port: port2,
      maxAgeSecs: 300,
    });
    server.start();

    try {
      const { status, body } = await getJson(
        `http://localhost:${port2}/health/oracle`
      );
      expect(status).toBe(503);
      expect((body as Record<string, unknown>).status).toBe("degraded");
      expect(
        (body as Record<string, unknown>).lastSuccessfulRunIso
      ).toBeNull();
    } finally {
      await server.stop();
    }
  });

  it("GET /health/oracle returns 200 after a simulated successful run", async () => {
    const port3 = await getAvailablePort();
    const oracle = buildOracle();

    // Directly set metrics to simulate a recent successful run.
    oracle.metrics.lastSuccessfulRunMs = Date.now();
    oracle.metrics.totalRuns = 3;
    oracle.metrics.pairs["USD/NGN"] = {
      lastSuccessTs: Date.now(),
      lastError: null,
      errorCount: 0,
      provider: "primary",
    };

    const server = createHealthServer(oracle, {
      port: port3,
      maxAgeSecs: 300,
    });
    server.start();

    try {
      const { status, body } = await getJson(
        `http://localhost:${port3}/health/oracle`
      );
      expect(status).toBe(200);
      const b = body as Record<string, unknown>;
      expect(b.status).toBe("healthy");
      expect(typeof b.lastSuccessfulRunIso).toBe("string");
      expect(b.totalRuns).toBe(3);
      const pairs = b.pairs as Record<
        string,
        { status: string; provider: string }
      >;
      expect(pairs["USD/NGN"]!.status).toBe("ok");
      expect(pairs["USD/NGN"]!.provider).toBe("primary");
    } finally {
      await server.stop();
    }
  });

  it("GET /health/oracle returns 503 when last run is older than 2× maxAgeSecs", async () => {
    const port4 = await getAvailablePort();
    const oracle = buildOracle();

    // Simulate a run that happened 601 s ago with maxAgeSecs=300 (threshold=600).
    oracle.metrics.lastSuccessfulRunMs = Date.now() - 601_000;

    const server = createHealthServer(oracle, {
      port: port4,
      maxAgeSecs: 300,
    });
    server.start();

    try {
      const { status, body } = await getJson(
        `http://localhost:${port4}/health/oracle`
      );
      expect(status).toBe(503);
      expect((body as Record<string, unknown>).status).toBe("degraded");
    } finally {
      await server.stop();
    }
  });

  it("returns 404 for unknown routes", async () => {
    const port5 = await getAvailablePort();
    const oracle = buildOracle();
    const server = createHealthServer(oracle, { port: port5, maxAgeSecs: 300 });
    server.start();

    try {
      const { status } = await getJson(
        `http://localhost:${port5}/not/a/path`
      );
      expect(status).toBe(404);
    } finally {
      await server.stop();
    }
  });
});
