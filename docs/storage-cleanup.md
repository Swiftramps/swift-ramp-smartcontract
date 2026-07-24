# Storage cleanup on cancel

Issue #13. `cancel()` removes an enrollment **and** every storage entry that
enrollment created, rather than removing one and leaving the other behind.

## What an enrollment writes

`register_identity` writes up to two entries:

| Key | Written when |
| :--- | :--- |
| `DataKey::IdentityRecord(user)` | always |
| `DataKey::QueueMembership(queue_id, user)` | only when a `queue_id` was supplied |

Deleting one without the other is what produces stale data. Two concrete
failure modes:

- **Orphaned membership.** Delete the identity but keep `QueueMembership` and the
  queue still lists a user who no longer has a record. `can_transfer` happens to
  reject that case today (it reads the identity first and returns `false` when it
  is missing), but the entry is invisible garbage that nothing will ever collect,
  and any future code that iterates memberships would treat it as real.
- **Identity claiming a queue it has left.** Delete the membership but keep the
  record and `IdentityRecord.queue_id` still points at a queue the user is not in.

`cancel()` therefore reads `queue_id` **back off the stored record** instead of
taking it as a parameter. A caller cannot pass a mismatched queue and silently
orphan the membership entry.

## Eager vs lazy cleanup

Both are defensible; this contract uses **eager** cleanup (delete in `cancel()`).

### Eager — what we do

Delete both entries inside the cancelling transaction.

- Storage footprint returns to zero for that user immediately, which is the
  behaviour the issue asks for.
- Reads stay simple: every reader can trust that a present entry is a live entry,
  so no reader needs a "…but is it cancelled?" branch. That property is worth a
  lot, because a missed filter in one reader is a correctness bug.
- On Soroban, entries you delete stop accruing rent and stop needing TTL bumps.
  An entry you keep is an entry you must keep paying to keep alive.
- Cost is bounded and predictable: an enrollment creates at most two entries, so a
  cancel removes at most two. There is no unbounded loop to run out of budget in.

The price: the cancelling transaction pays for the deletes, and the on-chain
history of the enrollment is gone from *state*. It is still in the ledger's event
and transaction history, so this costs convenience, not auditability.

### Lazy — what we did not do

Mark the record cancelled (a tombstone) and sweep later, or let the TTL lapse.

- Cheaper at cancel time and preserves the record for direct state queries.
- But every reader must now filter tombstones, and the entries keep costing rent
  until something collects them. "Something collects them" means writing and
  operating a sweeper, which is a second moving part that can fall behind.
- Letting TTL expiry do the collecting is the worst of both: the data lingers for
  the full TTL, and archived-but-not-deleted entries can still be restored.

Lazy cleanup earns its keep when deletion is expensive or unbounded — for example
a user with thousands of associated rows, where deleting them all in one call
would blow the resource budget. That is not the shape here: the work is O(1), so
the simplicity of eager deletion wins.

## Idempotency

`cancel()` on an identity that is already gone is a **no-op, not a panic**.
Transactions get retried, and a retry that fails purely because the first attempt
succeeded is a bad failure mode for a cleanup operation. The trade-off is that
cancelling a typo'd address reports success; callers that need to distinguish
"cancelled something" from "there was nothing there" should check
`get_identity_status` first.

## Re-enrollment

Because nothing is tombstoned, `register_identity` after a `cancel()` is an
ordinary fresh registration — there is no leftover entry to collide with and no
"previously cancelled" state to clear. `test_cancel_then_reregister_restores_access`
pins this.

## Tests

In `contracts/lineproof-identity/src/lib.rs`. They assert against **raw storage**
via `env.as_contract(...)` + `storage().instance().has(...)`, rather than through a
getter, so they verify what is actually persisted:

- `test_cancel_removes_identity_and_membership` — both keys gone
- `test_cancel_without_queue_removes_identity` — the no-queue path
- `test_cancel_leaves_other_users_untouched` — cleanup is scoped to one user
- `test_cancel_is_idempotent` — a second cancel does not panic
- `test_cancel_of_unknown_user_is_a_noop`
- `test_cancelled_user_cannot_transfer` — reads as unregistered afterwards
- `test_cancel_then_reregister_restores_access`
