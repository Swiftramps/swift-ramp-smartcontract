# Identity Status Semantics

This document defines the `IdentityStatus` lifecycle in the `lineproof-identity`
contract and how it gates transfer eligibility.

## The `IdentityStatus` type

| Variant     | u32 index | Meaning                                                        |
|-------------|-----------|----------------------------------------------------------------|
| `Active`    | `0`       | Registered identity in good standing; eligible for transfers.  |
| `Revoked`   | `1`       | Identity revoked by an admin; **never** eligible for transfers. |
| `Suspended` | `2`       | Reserved for future use; currently behaves as non-transferable. |

The status is stored inside the `IdentityRecord`:

```rust
pub struct IdentityRecord {
    pub status: IdentityStatus,
    pub queue_id: Option<Symbol>,
    pub registered_at: u64,
}
```

## Lifecycle

| Transition              | Function                | Notes                                             |
|-------------------------|-------------------------|---------------------------------------------------|
| none → `Active`         | `initialize` / `register_identity` | Admin bootstrapped on init; identities start `Active`. |
| `Active` → `Revoked`    | `revoke_identity`       | Admin must be `Active` and authorise. Fails with `AlreadyRevoked` if already revoked. |
| any → `Cancelled`       | `cancel`                | Removes the identity + queue membership entirely; reads as *unregistered*, not `Revoked`. |

## `can_transfer()` semantics

`can_transfer(from, to, queue_id)` returns `true` **only** when *every* check
passes:

1. `from == to` → self-transfer is always allowed.
2. Both `from` and `to` have an `IdentityRecord`.
3. **Neither party's status is `Revoked`** — a revoked identity is blocked even
   when it is still a member of the queue, and the other party being revoked
   also blocks the transfer.
4. Both `from` and `to` are members of `queue_id` (see
   `docs/queue-membership.md`).

### Revoked semantics in detail

- The `Revoked` check runs **before** queue membership, so a revoked identity
  can never appear transferable, regardless of membership state.
- Revocation does not delete the `IdentityRecord`; it flips its status. The
  record remains readable via `get_identity_status` (returning `Revoked`) so
  the revocation itself stays visible to the audit trail.
- To fully remove an identity (record *and* membership), use `cancel`.

## `get_identity_status(user)`

Returns the stored `IdentityStatus`, or `Error::IdentityNotFound` when the
identity is not registered. This is the read the backend `GET
/identities/:address/status` endpoint proxies.

## Test coverage

- `test_can_transfer_revoked_identity` — revoked party cannot transfer even
  while in the queue.
- `test_cancelled_user_cannot_transfer` — removed identities read as
  unregistered, not lingering members.
- `test_get_identity_status` — status reflects active/revoked state.
- `test_typed_errors` — `AlreadyRevoked`, `AdminNotActive`, and
  `IdentityNotFound` error paths.
