# Preimage Format

This document defines the canonical binary preimage format used for commitment-based hash verification in SwiftRamp. Off-chain auditors and verifiers MUST reconstruct the hash exactly as specified here to replay on-chain commitments.

## Encoding

The preimage is a fixed-size 72-byte binary sequence, encoded in **big-endian** byte order for platform-independent determinism:

```
┌────────────────────────┬──────────────────────────┬──────────────────────────┐
│    enrolled_at (8 B)   │    identity (32 B)       │    queue_id (32 B)       │
│     big-endian u64     │    raw bytes              │    raw bytes              │
└────────────────────────┴──────────────────────────┴──────────────────────────┘
Offset: 0                 8                          40                         72
```

| Field        | Size (bytes) | Type          | Byte Order | Description                                     |
|-------------|-------------|---------------|------------|-------------------------------------------------|
| `enrolled_at` | 8           | `u64`         | Big-endian  | Unix timestamp (seconds) when the identity was enrolled, matching `IdentityRecord.registered_at` |
| `identity`    | 32          | `Address`     | N/A         | The raw 32-byte Soroban address of the identity |
| `queue_id`    | 32          | `Symbol`/raw  | N/A         | The 32-byte queue identifier; zero-padded on the right if shorter than 32 bytes |

## Hash Computation

The commitment hash is:

```
hash = SHA-256(preimage)
```

- SHA-256 is used (not Keccak-256).
- The output is a 32-byte digest.

## Example

Given the following values:

| Field        | Value (hex)                                                      |
|-------------|------------------------------------------------------------------|
| `enrolled_at` | `0x00000000678FDB40` (1,738,123,072 — a Unix timestamp) |
| `identity`    | `0xCAFEBABECAFEBABECAFEBABECAFEBABECAFEBABECAFEBABECAFEBABECAFEBABE` |
| `queue_id`    | `0x0000000000000000000000000000000000000000000000000000000000000000` |

The 72-byte preimage (hex) is:

```
00000000678FDB40 CAFEBABECAFEBABECAFEBABECAFEBABECAFEBABECAFEBABECAFEBABECAFEBABE 0000000000000000000000000000000000000000000000000000000000000000
```

## Verifier

An off-chain verifier implementation is available at:

```
swift-ramp-backend/src/verifier.ts
```

Run the verifier tests with:

```bash
cd swift-ramp-backend && npm test
```

## Platform Independence

- All multi-byte integers are encoded in **big-endian** (network byte order). This applies to `enrolled_at`.
- No floating-point or variable-length encoding is used.
- The preimage is fixed at exactly 72 bytes — no padding, no length prefixes, no variable fields.

## On-Chain Integration

Within a Soroban contract, the commitment hash is computed as:

```rust
use soroban_sdk::{BytesN, Bytes, Env};

fn compute_commitment(
    env: &Env,
    enrolled_at: u64,
    identity: &BytesN<32>,
    queue_id: &BytesN<32>,
) -> BytesN<32> {
    let mut preimage = Bytes::new(env);
    // 8 bytes big-endian enrolled_at
    preimage.append(&Bytes::from_array(env, &enrolled_at.to_be_bytes()));
    // 32 bytes identity
    preimage.append(&Bytes::from_array(env, &identity.to_array()));
    // 32 bytes queue_id
    preimage.append(&Bytes::from_array(env, &queue_id.to_array()));
    env.crypto().sha256(&preimage).into()
}
```

## Versioning

This is **v1** of the preimage format. Any future changes MUST increment the version and be documented in a new section here.
