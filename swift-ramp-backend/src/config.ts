/**
 * config.ts
 *
 * Centralised configuration loader for the SwiftRamp oracle backend.
 *
 * Secret key resolution order (most-preferred first):
 *   1. AWS Secrets Manager  — if AWS_SECRET_ARN is set
 *   2. HashiCorp Vault       — if VAULT_ADDR + VAULT_TOKEN + VAULT_SECRET_PATH
 *                              are all set
 *   3. Environment variable  — ORACLE_SECRET_KEY (dev / CI fallback)
 *
 * The key is validated on startup:
 *   • Must decode to a valid 32-byte Stellar Ed25519 secret seed (S…).
 *   • A warning is emitted if the process is running as root, since that
 *     implies the .env file may not be chmod 600.
 */

import { Keypair, Networks } from "@stellar/stellar-sdk";

// ── types ─────────────────────────────────────────────────────────────────────

export interface AppConfig {
  rpcUrl: string;
  networkPassphrase: string;
  contractId: string;
  /** Resolved and validated Stellar admin keypair */
  adminKeypair: Keypair;
  /** Push interval in seconds */
  intervalSecs: number;
  /** Max rate age the contract will accept, in seconds */
  maxAgeSecs: number;
  /** Currency pairs to maintain, e.g. [["USD","NGN"],["USD","KES"]] */
  currencyPairs: [string, string][];
}

// ── secret resolution ─────────────────────────────────────────────────────────

/**
 * Load the raw secret key string from the most preferred available source.
 * Throws if no source yields a non-empty value.
 */
export async function resolveSecretKey(): Promise<string> {
  // 1. AWS Secrets Manager
  const awsArn = process.env["AWS_SECRET_ARN"];
  if (awsArn) {
    try {
      const key = await loadFromAwsSecretsManager(awsArn);
      console.info("[config] admin key loaded from AWS Secrets Manager");
      return key;
    } catch (err) {
      console.warn(
        `[config] AWS Secrets Manager failed (${String(err)}); ` +
          `falling through to next source`
      );
    }
  }

  // 2. HashiCorp Vault
  const vaultAddr = process.env["VAULT_ADDR"];
  const vaultToken = process.env["VAULT_TOKEN"];
  const vaultPath = process.env["VAULT_SECRET_PATH"];
  if (vaultAddr && vaultToken && vaultPath) {
    try {
      const key = await loadFromVault(vaultAddr, vaultToken, vaultPath);
      console.info("[config] admin key loaded from HashiCorp Vault");
      return key;
    } catch (err) {
      console.warn(
        `[config] Vault failed (${String(err)}); ` +
          `falling through to env var`
      );
    }
  }

  // 3. Environment variable (dev / CI)
  const envKey = process.env["ORACLE_SECRET_KEY"];
  if (envKey && envKey.trim().length > 0) {
    console.info(
      "[config] admin key loaded from ORACLE_SECRET_KEY env var " +
        "(use a secrets manager in production)"
    );
    warnIfInsecureEnvironment();
    return envKey.trim();
  }

  throw new Error(
    "No admin secret key found. " +
      "Set AWS_SECRET_ARN, VAULT_ADDR+VAULT_TOKEN+VAULT_SECRET_PATH, " +
      "or ORACLE_SECRET_KEY."
  );
}

// ── AWS Secrets Manager integration ──────────────────────────────────────────

async function loadFromAwsSecretsManager(secretArn: string): Promise<string> {
  // Dynamic import keeps the AWS SDK out of the bundle when not needed.
  // @ts-expect-error — optional peer dependency
  const { SecretsManagerClient, GetSecretValueCommand } = await import(
    "@aws-sdk/client-secrets-manager"
  );
  const client = new SecretsManagerClient({});
  const response = await client.send(
    new GetSecretValueCommand({ SecretId: secretArn })
  );
  const raw: string = response.SecretString ?? "";
  if (!raw) throw new Error("SecretString is empty");
  // Support both plain-string secrets and JSON objects { "key": "S..." }
  try {
    const parsed = JSON.parse(raw) as Record<string, string>;
    const value = parsed["key"] ?? parsed["secret"] ?? parsed["ORACLE_SECRET_KEY"];
    if (value) return value;
  } catch {
    // Not JSON — treat the whole string as the key.
  }
  return raw.trim();
}

// ── HashiCorp Vault integration ───────────────────────────────────────────────

async function loadFromVault(
  vaultAddr: string,
  token: string,
  secretPath: string
): Promise<string> {
  const url = `${vaultAddr}/v1/${secretPath}`;
  const res = await fetch(url, {
    headers: { "X-Vault-Token": token },
  });
  if (!res.ok) {
    throw new Error(`Vault returned HTTP ${res.status} for path ${secretPath}`);
  }
  const body = (await res.json()) as {
    data?: { data?: Record<string, string>; key?: string };
  };
  // KV v2: data.data.key  |  KV v1: data.key
  const value =
    body.data?.data?.["key"] ??
    body.data?.data?.["ORACLE_SECRET_KEY"] ??
    body.data?.["key"];
  if (!value) throw new Error("Could not extract key from Vault response");
  return value;
}

// ── key validation ────────────────────────────────────────────────────────────

/**
 * Validate that `rawKey` is a well-formed Stellar Ed25519 secret seed.
 * Returns a `Keypair` on success, throws with a descriptive message on failure.
 */
export function validateAndLoadKeypair(rawKey: string): Keypair {
  if (!rawKey.startsWith("S")) {
    throw new Error(
      "Admin secret key must start with 'S' (Stellar Ed25519 seed format)"
    );
  }
  if (rawKey.length !== 56) {
    throw new Error(
      `Admin secret key has unexpected length ${rawKey.length} (expected 56)`
    );
  }
  try {
    return Keypair.fromSecret(rawKey);
  } catch (err) {
    throw new Error(`Admin secret key is not a valid Stellar keypair: ${String(err)}`);
  }
}

// ── environment security warnings ────────────────────────────────────────────

function warnIfInsecureEnvironment(): void {
  // On POSIX systems, UID 0 = root. Running as root means the .env file
  // is accessible to any process on the system without privilege escalation.
  if (typeof process.getuid === "function" && process.getuid() === 0) {
    console.warn(
      "[config] WARNING: process is running as root. " +
        "Ensure .env is chmod 600 and owned by root, or migrate to a " +
        "secrets manager (AWS Secrets Manager / HashiCorp Vault)."
    );
  }
}

// ── main config loader ────────────────────────────────────────────────────────

/**
 * Build the full `AppConfig` by resolving and validating all settings.
 * Throws on any missing required value or invalid key format.
 */
export async function loadConfig(): Promise<AppConfig> {
  const rawKey = await resolveSecretKey();
  const adminKeypair = validateAndLoadKeypair(rawKey);

  const network = (process.env["STELLAR_NETWORK"] ?? "testnet").toLowerCase();
  const isMainnet = network === "mainnet";

  const rpcUrl = isMainnet
    ? (process.env["STELLAR_RPC_URL"] ?? "https://soroban.stellar.org")
    : (process.env["STELLAR_RPC_URL"] ?? "https://soroban-testnet.stellar.org");

  const networkPassphrase = isMainnet ? Networks.PUBLIC : Networks.TESTNET;

  const contractId = process.env["SWAP_CONTRACT_ID"] ?? "";
  if (!contractId) {
    throw new Error("SWAP_CONTRACT_ID environment variable is required");
  }

  const intervalSecs = Number(process.env["ORACLE_INTERVAL_SECS"] ?? "300");
  const maxAgeSecs = Number(process.env["ORACLE_MAX_AGE_SECS"] ?? "3600");

  const pairsRaw = process.env["CURRENCY_PAIRS"] ?? "USD/NGN,USD/KES,USD/GHS,USD/ZAR";
  const currencyPairs: [string, string][] = pairsRaw
    .split(",")
    .map((p) => {
      const [from, to] = p.trim().split("/");
      if (!from || !to) throw new Error(`Invalid currency pair: "${p}"`);
      return [from.toUpperCase(), to.toUpperCase()] as [string, string];
    });

  if (intervalSecs >= maxAgeSecs) {
    console.warn(
      `[config] WARNING: ORACLE_INTERVAL_SECS (${intervalSecs}) >= ` +
        `ORACLE_MAX_AGE_SECS (${maxAgeSecs}). ` +
        `Rates will expire before the next push cycle.`
    );
  }

  return {
    rpcUrl,
    networkPassphrase,
    contractId,
    adminKeypair,
    intervalSecs,
    maxAgeSecs,
    currencyPairs,
  };
}
