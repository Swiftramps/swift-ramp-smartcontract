# Queue Membership Contract Interface

This document describes the queue-membership interface used by
`lineproof-identity` and how `can_transfer()` enforces it.

## Storage key

Membership is tracked with a single persistent storage key:

```rust
DataKey::QueueMembership(Symbol queue_id, Address user) -> bool
```

The key is `(queue_id, user)` and the stored value is the boolean `true` — the
presence of the key *is* the membership. There is no separate registry of
queues; a queue is any `Symbol` that identities have enrolled into.

## When membership is written

`register_identity(admin, user, queue_id: Option<Symbol>)`:

- Always writes `IdentityRecord(user)`.
- When `queue_id` is `Some(qid)`, additionally writes
  `QueueMembership(qid, user) -> true`.

So an identity registered *with* a queue is a member; an identity registered
*without* one has no membership anywhere.

## When membership is removed

`cancel(admin, user)` removes the membership **and** the identity record,
reading the queue off the stored `IdentityRecord.queue_id` so the caller cannot
pass a mismatched `queue_id` and orphan the membership entry. Cleanup is eager
and idempotent — see `docs/storage-cleanup.md`.

## How `can_transfer()` enforces it

`can_transfer(from, to, queue_id)` requires **both** parties to be members of
`queue_id`:

```rust
let from_in_queue: Option<bool> = env
    .storage()
    .instance()
    .get(&DataKey::QueueMembership(queue_id.clone(), from.clone()));
let to_in_queue: Option<bool> = env
    .storage()
    .instance()
    .get(&DataKey::QueueMembership(queue_id, to.clone()));

from_in_queue.unwrap_or(false) && to_in_queue.unwrap_or(false)
```

An identity that is not a member (or whose membership was removed by `cancel`)
reads as `false`. Combined with the validity and revoked checks, `can_transfer`
returns `true` **only** when the identity is valid, non-revoked, **and** in the
queue.

### Why the explicit membership check matters

Without it, any two *registered* identities could transfer regardless of the
queue they belong to — a user enrolled in the remittance queue could transact
with someone enrolled in a different queue. The check scopes every transfer to
the compliance queue both parties actually joined.

## Test coverage

- `test_can_transfer_reads_identity_record` — both parties in the queue → allowed.
- `test_can_transfer_not_in_queue` — neither party in the queue → blocked.
- `test_can_transfer_unregistered_identity` — missing identity record → blocked.
- `test_can_transfer_mixed_membership` — one party in the queue, the other not → blocked.
- `test_cancelled_user_cannot_transfer` — membership removed on cancel → blocked.
