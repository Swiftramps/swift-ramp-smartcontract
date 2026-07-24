import {
  buildPreimage,
  hashPreimage,
  verifyCommitment,
  PREIMAGE_SIZE,
} from "../verifier";

describe("preimage format", () => {
  it("builds a 72-byte preimage from components", () => {
    const enrolledAt = 1_738_123_072;
    const identity = Buffer.alloc(32, 0xca);
    const queueId = Buffer.alloc(32, 0x00);

    const preimage = buildPreimage(enrolledAt, identity, queueId);

    expect(preimage.length).toBe(72);

    // enrolled_at at offset 0: 8 bytes big-endian
    expect(preimage.readBigUInt64BE(0)).toBe(BigInt(enrolledAt));

    // identity at offset 8
    for (let i = 0; i < 32; i++) {
      expect(preimage[8 + i]).toBe(0xca);
    }

    // queue_id at offset 40
    for (let i = 0; i < 32; i++) {
      expect(preimage[40 + i]).toBe(0x00);
    }
  });

  it("rejects wrong-sized identity", () => {
    expect(() =>
      buildPreimage(0, Buffer.alloc(16), Buffer.alloc(32)),
    ).toThrow("identity must be 32 bytes");
  });

  it("rejects wrong-sized queueId", () => {
    expect(() =>
      buildPreimage(0, Buffer.alloc(32), Buffer.alloc(16)),
    ).toThrow("queueId must be 32 bytes");
  });

  it("rejects wrong-sized preimage in hashPreimage", () => {
    expect(() => hashPreimage(Buffer.alloc(8))).toThrow(
      "preimage must be exactly 72 bytes",
    );
  });
});

describe("hash computation", () => {
  it("produces a deterministic 32-byte hash from the same preimage", () => {
    const enrolledAt = 1_700_000_000;
    const identity = Buffer.alloc(32, 0xab);
    const queueId = Buffer.alloc(32, 0x01);

    const h1 = hashPreimage(buildPreimage(enrolledAt, identity, queueId));
    const h2 = hashPreimage(buildPreimage(enrolledAt, identity, queueId));

    expect(h1).toEqual(h2);
    expect(h1.length).toBe(32);
  });

  it("produces different hashes for different enrolled_at values", () => {
    const identity = Buffer.alloc(32, 0xab);
    const queueId = Buffer.alloc(32, 0x01);

    const h1 = hashPreimage(buildPreimage(1_700_000_000, identity, queueId));
    const h2 = hashPreimage(buildPreimage(1_800_000_000, identity, queueId));

    expect(h1).not.toEqual(h2);
  });

  it("produces different hashes for different identities", () => {
    const enrolledAt = 1_700_000_000;
    const queueId = Buffer.alloc(32, 0x01);

    const h1 = hashPreimage(
      buildPreimage(enrolledAt, Buffer.alloc(32, 0xab), queueId),
    );
    const h2 = hashPreimage(
      buildPreimage(enrolledAt, Buffer.alloc(32, 0xcd), queueId),
    );

    expect(h1).not.toEqual(h2);
  });

  it("produces different hashes for different queue_ids", () => {
    const enrolledAt = 1_700_000_000;
    const identity = Buffer.alloc(32, 0xab);

    const h1 = hashPreimage(
      buildPreimage(enrolledAt, identity, Buffer.alloc(32, 0x01)),
    );
    const h2 = hashPreimage(
      buildPreimage(enrolledAt, identity, Buffer.alloc(32, 0x02)),
    );

    expect(h1).not.toEqual(h2);
  });
});

describe("verifyCommitment", () => {
  it("returns true when hash matches", () => {
    const enrolledAt = 1_738_123_072;
    const identity = Buffer.alloc(32, 0xca);
    const queueId = Buffer.alloc(32, 0x00);

    const preimage = buildPreimage(enrolledAt, identity, queueId);
    const expectedHash = hashPreimage(preimage);

    expect(
      verifyCommitment(enrolledAt, identity, queueId, expectedHash),
    ).toBe(true);
  });

  it("returns false when hash does not match", () => {
    const enrolledAt = 1_738_123_072;
    const identity = Buffer.alloc(32, 0xca);
    const queueId = Buffer.alloc(32, 0x00);

    const wrongHash = Buffer.alloc(32, 0xff);

    expect(
      verifyCommitment(enrolledAt, identity, queueId, wrongHash),
    ).toBe(false);
  });
});
