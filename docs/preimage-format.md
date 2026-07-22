# Off-Chain Proof Hash Preimage Specification

This document details the binary encoding format, field ordering, byte alignment, and hashing algorithm used to produce the `proof_hash` for off-chain auditors and compliance verification teams.

---

## Overview

The `proof_hash` is a 32-byte cryptographic digest computed over a structured preimage containing participant identity information, queue membership, and ledger timestamps. It guarantees non-repudiation, tamper-resistance, and deterministic verification of audit trail events.

- **Hash Function**: `SHA-256` (producing a 256-bit / 32-byte hash, typically represented as a 64-character hexadecimal string with a `0x` prefix).
- **Endianness**: Big-Endian (`BE`) network byte order for numerical fields.
- **Encoding Scheme**: Compact binary concatenation of standard XDR byte representations followed by big-endian integer fields.

---

## Preimage Field Layout Table

The binary preimage buffer is constructed by concatenating the serialized fields in the exact order specified in the layout table below:

| Field Name | Type | Serialization / Encoding | Byte Length | Description |
|---|---|---|---|---|
| `identity` | `Address` | Soroban XDR `Address` byte representation (`to_xdr()`) | Variable (36 bytes) | Participant's Stellar public key address |
| `queue_id` | `Symbol` | Soroban XDR `Symbol` byte representation (`to_xdr()`) | Variable (12–16 bytes) | Queue identifier symbol (e.g., `queueA`) |
| `timestamp` | `u64` | 8-byte Big-Endian Unsigned Integer (`to_be_bytes()`) | 8 bytes | Ledger sequence number or Unix timestamp |

---

## Off-Chain Hash Reproduction

### Hash Computation Formula

$$\text{Preimage} = \text{XDR}(\text{identity}) \mathbin{\Vert} \text{XDR}(\text{queue\_id}) \mathbin{\Vert} \text{BigEndianU64}(\text{timestamp})$$

$$\text{proof\_hash} = \text{SHA256}(\text{Preimage})$$

---

## Implementation Code Snippets

### Python Snippet (Off-Chain Verification)

```python
import hashlib
import struct
from stellar_sdk import Address, Symbol, xdr

def compute_proof_hash_python(identity_str: str, queue_id_str: str, timestamp: int) -> str:
    """
    Reproduces the exact on-chain SHA-256 proof_hash off-chain.
    """
    # 1. Encode Identity Address to XDR bytes
    address_xdr = Address(identity_str).to_xdr_bytes()

    # 2. Encode Queue Symbol to XDR bytes
    symbol_xdr = xdr.Symbol(queue_id_str.encode('utf-8')).to_xdr_bytes()

    # 3. Encode timestamp as 8-byte Big-Endian integer
    timestamp_be = struct.pack(">Q", timestamp)

    # 4. Concatenate binary payload
    payload = address_xdr + symbol_xdr + timestamp_be

    # 5. Compute SHA-256 hash
    digest = hashlib.sha256(payload).hexdigest()
    return f"0x{digest}"

# Example Usage:
if __name__ == "__main__":
    addr = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF"
    qid = "queueA"
    ts = 1700000000
    proof_hash = compute_proof_hash_python(addr, qid, ts)
    print(f"Computed Proof Hash: {proof_hash}")
```

### TypeScript / Node.js Snippet (Off-Chain Verification)

```typescript
import { createHash } from 'crypto';
import { Address, xdr } from '@stellar/stellar-sdk';

export function computeProofHash(
  identityAddr: string,
  queueId: string,
  timestamp: bigint
): string {
  // 1. Serialize Address to XDR
  const addressXdr = Address.fromString(identityAddr).toScVal().toXDR();

  // 2. Serialize Symbol to XDR
  const symbolXdr = xdr.ScVal.scvSymbol(queueId).toXDR();

  // 3. Serialize timestamp to Big-Endian 64-bit Buffer
  const timestampBuf = Buffer.alloc(8);
  timestampBuf.writeBigUInt64BE(timestamp, 0);

  // 4. Concatenate byte buffers
  const payload = Buffer.concat([addressXdr, symbolXdr, timestampBuf]);

  // 5. SHA-256 Digest
  const hashHex = createHash('sha256').update(payload).digest('hex');
  return `0x${hashHex}`;
}
```

---

## Cryptographic Security & Determinism Notes

1. **SHA-256 Usage**: SHA-256 guarantees second-preimage resistance and collision resistance for on-chain identity compliance.
2. **Determinism**: Given the identical inputs `(identity, queue_id, timestamp)`, the off-chain calculation produces the exact 32-byte digest emitted by the `LineproofIdentity` contract's `compute_proof_hash()` method.
