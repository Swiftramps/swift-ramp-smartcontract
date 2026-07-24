import { createHash } from "crypto";

export const PREIMAGE_SIZE = 72;
export const HASH_ALGORITHM = "sha256";

/**
 * Compute the commitment hash for a given preimage.
 *
 * @param preimage - Exactly 72 bytes: 8 BE enrolled_at | 32 identity | 32 queue_id
 * @returns 32-byte SHA-256 digest
 */
export function hashPreimage(preimage: Buffer): Buffer {
  if (preimage.length !== PREIMAGE_SIZE) {
    throw new Error(
      `preimage must be exactly ${PREIMAGE_SIZE} bytes, got ${preimage.length}`,
    );
  }
  return createHash(HASH_ALGORITHM).update(preimage).digest();
}

/**
 * Build the 72-byte canonical preimage from its components.
 *
 * @param enrolledAt - Unix timestamp in seconds (u64)
 * @param identity   - 32-byte identity address
 * @param queueId    - 32-byte queue identifier
 * @returns 72-byte preimage buffer
 */
export function buildPreimage(
  enrolledAt: number,
  identity: Buffer,
  queueId: Buffer,
): Buffer {
  if (identity.length !== 32) {
    throw new Error(`identity must be 32 bytes, got ${identity.length}`);
  }
  if (queueId.length !== 32) {
    throw new Error(`queueId must be 32 bytes, got ${queueId.length}`);
  }

  const buf = Buffer.alloc(PREIMAGE_SIZE);

  // 8 bytes big-endian enrolled_at at offset 0
  buf.writeBigUInt64BE(BigInt(enrolledAt), 0);

  // 32 bytes identity at offset 8
  identity.copy(buf, 8);

  // 32 bytes queue_id at offset 40
  queueId.copy(buf, 40);

  return buf;
}

/**
 * Verify that a known preimage produces the expected commitment hash.
 *
 * @param enrolledAt    - Unix timestamp in seconds (u64)
 * @param identity      - 32-byte identity address
 * @param queueId       - 32-byte queue identifier
 * @param expectedHash  - The 32-byte hash to verify against
 * @returns true if the computed hash matches expectedHash
 */
export function verifyCommitment(
  enrolledAt: number,
  identity: Buffer,
  queueId: Buffer,
  expectedHash: Buffer,
): boolean {
  const preimage = buildPreimage(enrolledAt, identity, queueId);
  const computed = hashPreimage(preimage);
  return computed.equals(expectedHash);
}
