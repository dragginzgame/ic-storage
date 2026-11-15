# Single-root live suspension

This private host experiment joins the allocation-ledger image to the original
live arena and unique parking token. It admits one `Vec<u64, &Arena>` root. It
does not restore an allocation domain after an IC upgrade or define a product API.

## Admission and custody

Suspension validates the original token against the whole live ledger and requires
that its allocation account for every allocated block. A zero-capacity root
requires an empty ledger. Additional live, parked or forgotten allocations refuse;
they cannot silently disappear from the inventory. Refusal returns the original
token and changes neither the arena nor its ledger.

Successful admission consumes the token into a suspension object, records the
checked ledger image and sets an independent arena flag. Allocation and ordinary
token reopening then refuse. A legitimate deallocation cannot occur: the sole
allocated owner was consumed. An unsafe caller violating that condition aborts.
Other zero-capacity vectors hold no allocation and cannot allocate while frozen.

Resume checks the image, compares its decoded metadata with the still-live ledger
and rechecks the original token and complete inventory. Only then does it consume
the token, clear the flag and invoke the existing live-domain reopening mechanism.
There is no callback, allocation or new input between validation and reopening.
An unexpected refusal from the repeated admission check restores the flag. A
successful resume permits normal growth, removal, shrinking and reclamation;
a second resume refuses.

An explicit [transfer](transfer.md) is now a separate terminal operation: it
consumes the original token into a private release record while keeping the
source frozen. Only consuming that released source can hand off its actual Box.
Dropping or forgetting suspension without this operation never grants transfer.

The image never installs bookkeeping or mints ownership. Two arenas with different
values can have identical ledger images. Swapping those images still reopens each
original token's own allocation. Consistently changing the live ledger and image
to a new allocation generation also fails against the original token.

## Forgetting and limits

There is no suspension destructor on which safety depends. Dropping or forgetting
the object leaves the arena frozen, including for an empty root. Lost authority
can leak capacity or block further access but cannot authorize reuse. The whole
arena can still be discarded once its borrow lifetime ends, as described in the
[allocation boundary](allocation-boundary.md).

The flag, token and live arena are transient host objects, not a retained ABI.
The inventory is complete only for this one-vector subset. It supplies no general
graph traversal, nested owner accounting, persisted root identity, snapshot
publication protocol or cross-instance pointer-validity argument. The
[ledger format](ledger-format.md) remains metadata, not a recovery checkpoint.
The portable logical checkpoint is still the path for reconstructing values
without depending on the original native allocation domain.

The [checkpoint handoff tests](checkpoint-format.md#handoff-from-a-frozen-live-source)
now connect export, suspension, cancellation and fresh-arena import. They test
recovery after losing the original arena and token, without extending resume
authority across that loss or implementing a durable migration coordinator.

## Evidence

Ten focused tests cover single-use resume and reclamation, additional live and
parked owners, forgotten owners, dropped/forgotten suspension, damaged and changed
images, coherently changed ledger generations, identical images across domains,
empty roots and invalid root descriptors. Run them with the allocation suite:

```sh
cargo clippy --manifest-path qualification/Cargo.toml \
  -p ic-storage-native-feasibility --target-dir target/qualification \
  --locked --offline --test allocation_tests -- -D warnings
cargo test --manifest-path qualification/Cargo.toml \
  -p ic-storage-native-feasibility --target-dir target/qualification \
  --locked --offline --test allocation_tests
```
