# Audit Event Schemas

This document specifies the event schemas emitted by the `lineproof-identity`
contract. Events are the **primary audit surface** of the compliance system —
the backend indexer streams them off-chain, normalises them, and serves them as
an audit trail (`GET /audit/:identity`). Every field type and naming choice here
is therefore part of the contract between the contract and its consumers.

---

## Topic Convention

All audit events share a two-element topic layout:

| Topics index | Type      | Description                                        |
|--------------|-----------|----------------------------------------------------|
| `topics[0]`  | `Symbol`  | Event type tag (`enrolled`, `cancelled`, `identity_updated`) |
| `topics[1]`  | `Address` | The participant's Stellar public key (`G…` / `C…`, 56 chars) |

Keeping the participant address as a severable topic makes it indexable
on-chain so consumers can filter by identity without decoding the data payload.

---

## `Enrolled`

Emitted **exactly once** when a new identity is registered with
`register_identity`.

**Topics**

| Index | Type      | Value       |
|-------|-----------|-------------|
| 0     | `Symbol`  | `enrolled`  |
| 1     | `Address` | `user`      |

**Data**

| Field          | Type       | Description                                                  |
|----------------|------------|--------------------------------------------------------------|
| `proof_hash`   | `BytesN<32>` | 32-byte SHA-256 digest (64 hex chars) anchoring the audit trail. Computed via `compute_proof_hash(identity, queue_id, timestamp)`. |
| `queue_id`     | `Symbol`   | The compliance queue the identity was enrolled into (`None` if none). |
| `registered_at`| `u64`      | Ledger sequence at registration.                              |

**Semantics**

- One `Enrolled` event per successful registration; never replayed.
- `proof_hash` ties the on-chain state update to the off-chain proof generated
  by the enrollment flow, closing the audit loop.

---

## `Cancelled`

Emitted **once per `cancel` call** that actually removes an identity (a no-op
cancel of an already-gone identity emits nothing).

**Topics**

| Index | Type      | Value        |
|-------|-----------|--------------|
| 0     | `Symbol`  | `cancelled`  |
| 1     | `Address` | `user`       |

**Data**

| Field         | Type       | Description                                          |
|---------------|------------|------------------------------------------------------|
| `proof_hash`  | `BytesN<32>` | The **original** enrollment's proof hash being cancelled, so a consumer can correlate the cancellation to its enrollment. |
| `cancelled_at`| `u64`      | Ledger sequence at cancellation.                      |
| `cancelled_by`| `Address`  | The admin address that authorised the cancellation.   |

**Semantics**

- The cancelling admin must be `Active` and must authorise the call.
- Cleanup is eager — identity record and queue membership are removed — and
  cancellation is idempotent (see `docs/storage-cleanup.md`).

---

## `IdentityUpdated`

Emitted when an identity's record transitions state (e.g. via `revoke_identity`).

**Topics**

| Index | Type      | Value               |
|-------|-----------|---------------------|
| 0     | `Symbol`  | `identity_updated`  |
| 1     | `Address` | `user`              |

**Data**

| Field       | Type            | Description                                      |
|-------------|-----------------|--------------------------------------------------|
| `status`    | `IdentityStatus` | New status — `Active` (`0`), `Revoked` (`1`), `Suspended` (`2`) — encoded as its u32 variant index. |
| `updated_at`| `u64`           | Ledger sequence at the update.                    |

**Semantics**

- Emitted only when the status actually changes (revoking an already-revoked
  identity fails with `AlreadyRevoked` and emits nothing).
- `IdentityStatus` is identical to the type stored in `IdentityRecord`; the
  string forms consumers may see are `active`, `revoked`, and `suspended`.

---

## Field Type Reference

| Type         | Encoding / Constraint                         |
|--------------|-----------------------------------------------|
| `Symbol`     | Soroban `Symbol` (≤ 32 bytes)                 |
| `Address`    | Stellar public key, `[GC][A-Z0-9]{55}` (56 chars) |
| `BytesN<32>` | 32-byte digest; hex-rendered as 64 characters (optional `0x` prefix) |
| `u64`        | 8-byte unsigned integer, ledger sequence      |

## Normalisation Notes for Consumers

The backend indexer (`src/lib/auditEventParser.ts`) normalises a few aliases so
the contract can spell the event tag either way:

- `enrolled` / `enroll` / `enrollment` → `enrolled`
- `cancelled` / `canceled` / `cancel` → `cancelled`

`proof_hash` is read from the data payload as `proof_hash`, `proofHash`, or
`hash`, or from the raw payload when the data *is* the hash itself.