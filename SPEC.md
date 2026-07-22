# dig-collateral-coin — SPEC

Normative contract for the `dig-collateral-coin` crate. This document describes what the crate
IS and what a consumer MUST/SHOULD do to use it correctly; an independent reimplementation of a
consumer against this crate can be built from this document alone. It is not a tutorial — see
`README.md` for usage walkthroughs.

## 1. Overview & scope

`dig-collateral-coin` is a **thin re-export crate**. It owns no CLVM puzzles, no coin-construction
logic, and no constants of its own; it forwards a minimal, curated surface from the upstream
`datalayer_driver` crate (which itself builds on `chia_wallet_sdk`) so a consumer can work with
`$DIG` CAT coins and `$DIG` collateral/mirror coins without depending on the full upstream API.

Because the crate is a re-export shim, this SPEC documents the contract at the surface this crate
vends and verifies (its public API, and the identity/behavioral guarantees the README documents for
that API). Byte-level CLVM puzzle definitions and the exact tree-hash derivation formula for morphed
IDs live upstream in `datalayer_driver`/`chia_wallet_sdk`, which are outside this repository; where
this SPEC cannot verify a detail directly, it says so explicitly rather than inventing it.

## 2. Public API surface (normative)

The crate's public surface is EXACTLY the re-exports in `src/lib.rs`. A consumer MUST NOT rely on
any other path into `datalayer_driver`/`chia_wallet_sdk` through this crate; only the following
names are part of this crate's contract:

```rust
pub use datalayer_driver::{
    Bytes32, CoinState, DigCoin, DigCollateralCoin, Peer, PublicKey, SpendBundle, connect_random,
    get_fee_estimate, sign_coin_spends, wallet::broadcast_spend_bundle,
};
pub use num_bigint;
```

- **Types:** `Bytes32`, `CoinState`, `DigCoin`, `DigCollateralCoin`, `Peer`, `PublicKey`,
  `SpendBundle`.
- **Functions:** `connect_random`, `get_fee_estimate`, `sign_coin_spends`,
  `broadcast_spend_bundle` (re-exported from `datalayer_driver::wallet`).
- **Crate re-export:** `num_bigint`, so a consumer can construct `num_bigint::BigInt` epoch values
  without an explicit dependency of its own.

A crate release that removes or renames any of these items is a **breaking change** (major version
bump, per the ecosystem's SemVer rule) and MUST be called out as such. Adding new re-exports is
additive (minor).

## 3. Canonical `$DIG` asset identity

The crate vends `DigCoin`/`DigCollateralCoin`, whose validation logic (upstream, in
`datalayer_driver`) checks a coin's CAT asset id against the canonical mainnet `$DIG` TAIL hash,
`datalayer_driver::wallet::DIG_ASSET_ID`. This value MUST equal, byte-for-byte:

```
a406d3a9de984d03c9591c10d917593b434d5263cabe2b42f6b367df16832f81
```

This is a byte-exact, ecosystem-wide canonical value (see the superproject `SYSTEM.md` and the
`docs.dig.net` `$DIG` CAT payment protocol page) — it MUST NOT drift across any crate or repo that
surfaces it. This crate does not define the value; it re-exposes it via `DigCoin`/
`DigCollateralCoin` validation.

**Conformance:** `tests/conformance.rs::dig_asset_id_matches_protocol_spec` pins
`datalayer_driver::wallet::DIG_ASSET_ID` against the spec hash above at test time, so an upstream
`datalayer-driver` version bump that silently changed the asset id would fail this crate's test
suite before it could reach a consumer. `tests/conformance.rs::vends_dig_cat_coin_types` is a
compile-time guard that the `DigCoin`/`DigCollateralCoin` re-exports continue to exist.

## 4. `$DIG` CAT coin contract — `DigCoin`

`DigCoin` proves a Chia coin is a bona fide `$DIG` CAT (Chia Asset Token) and exposes the parsed
CAT driver so it can be spent.

- `DigCoin::puzzle_hash(wallet_puzzle_hash: Bytes32) -> Bytes32` — computes the `$DIG` CAT outer
  puzzle hash for a given wallet owner puzzle hash. A consumer MAY call this without network access
  to predict where `$DIG` sent to a given wallet PH will land.
- `DigCoin::from_coin_state(peer: &Peer, coin_state: &CoinState) -> Result<Self, WalletError>` —
  given a `CoinState` fetched from a full node, parses the parent spend and validates:
  - the coin's asset id equals `DIG_ASSET_ID` (§3), and
  - the CAT lineage proof is present and valid.
  A consumer MUST treat a returned `Err` as "this coin is not a verified `$DIG` CAT" and MUST NOT
  assume a coin is `$DIG` without going through this validation (or the equivalent
  `DigCoin::from_coin`).
- `DigCoin::from_coin(peer: &Peer, coin: &Coin, created_height: u32) -> Result<Self, WalletError>` —
  lower-level constructor for callers that already hold a `Coin` + its creation height.
- `DigCoin::cat(&self) -> Cat` — returns the underlying `chia_wallet_sdk` CAT driver for building
  spends.

## 5. Collateral coin contract — `DigCollateralCoin`

A **collateral coin** is `$DIG` locked by a `P2Parent` puzzle layer for a given DIG store; it
secures the store and is reclaimable by its creator.

- **Control layer:** the coin is controlled by the standard Chia P2 (synthetic public key) layer.
  The SAME synthetic key pair used to CREATE a collateral coin MUST be used to later RECLAIM it —
  `spend()` (below) validates the coin's `parent_inner_puzzle_hash` against the hash of the caller's
  synthetic P2 layer, and returns an error (`WalletError`) when it does not match.
- `DigCollateralCoin::create_collateral(dig_coins: Vec<DigCoin>, amount: u64, store_id: Bytes32,
  synthetic_key: PublicKey, fee_coins: Vec<Coin>, fee: u64) -> Result<Vec<CoinSpend>, WalletError>` —
  builds (but does not sign or broadcast) the spends that create a collateral coin for `store_id`,
  locked to `synthetic_key`, funded from `dig_coins`, with an XCH `fee` covered by `fee_coins`.
- `DigCollateralCoin::from_coin_state(peer: &Peer, coin_state: CoinState) -> Result<Self,
  WalletError>` — validates that a coin state is an unspent, correctly-locked `$DIG`
  collateral/mirror coin (the P2Parent puzzle over the expected synthetic-key layer) and captures its
  memos for store/mirror interpretation.
- `DigCollateralCoin::coin(&self) -> Coin` / `DigCollateralCoin::proof(&self) -> LineageProof` —
  accessors for the underlying coin and the lineage proof used to prove control and spendability.
- `DigCollateralCoin::spend(&self, synthetic_key: PublicKey, fee_coins: Vec<Coin>, fee: u64) ->
  Result<Vec<CoinSpend>, WalletError>` — builds the spends that return a collateral (or mirror) coin
  the caller controls back to their standard P2 synthetic key (the reclaim path). MUST be signed with
  the private key matching `synthetic_key` before broadcast.

A consumer that discovers a candidate collateral coin (e.g. by the morphed hint, §7) MUST confirm
ownership by comparing `proof().parent_inner_puzzle_hash` against the hash of its own synthetic P2
layer before treating the coin as its own — `from_coin_state` proves the coin is a validly-locked
collateral/mirror coin, not that the CALLER owns it.

## 6. Mirror coin contract

A **mirror coin** uses the same `P2Parent`-locked construction as a collateral coin, with two
additions:

- **Epoch tagging:** mirror coins are tied to a specific epoch, represented as `num_bigint::BigInt`
  (re-exported by this crate, §2), so a consumer can express arbitrarily large epoch values without
  a upstream `num-bigint` dependency of its own.
- **Mirror URL memos:** mirror coins carry a list of UTF-8 mirror URLs in their coin memos (§8),
  used to advertise replication endpoints for the associated store.

- `DigCollateralCoin::create_mirror(dig_coins: Vec<DigCoin>, amount: u64, store_id: Bytes32,
  mirror_urls: Vec<String>, epoch: BigInt, synthetic_key: PublicKey, fee_coins: Vec<Coin>, fee: u64)
  -> Result<Vec<CoinSpend>, WalletError>` — builds the spends that create a mirror coin for
  `store_id`/`epoch`, carrying `mirror_urls` in its memos, locked to `synthetic_key`.
- Reclaim uses the same `spend()` entry point as collateral coins (§5) — the crate does not
  distinguish collateral vs. mirror coins at the reclaim call site; the distinction lives in how the
  coin was created (`create_collateral` vs. `create_mirror`) and in its memo layout (§8).

## 7. Morphed-ID namespacing

Store launcher IDs are **morphed** into distinct namespaces (tree hashes) so that on-chain discovery
hints for collateral vs. mirror coins of the SAME store do not collide, and so that mirror coins of
different epochs of the same store are independently discoverable:

- `DigCollateralCoin::morph_store_launcher_id_for_collateral(store_id: Bytes32) -> Bytes32` —
  derives the hint namespace used for collateral-coin discovery for `store_id`.
- `DigCollateralCoin::morph_store_launcher_id_for_mirror(store_id: Bytes32, epoch: &BigInt) ->
  Bytes32` — derives the hint namespace used for mirror-coin discovery for `store_id` at `epoch`.

A consumer discovers existing collateral/mirror coins by computing the appropriate morphed hint and
querying a full node for unspent coin states carrying that hint (e.g.
`datalayer_driver::get_unspent_coin_states_by_hint`), then validating each candidate via
`from_coin_state` (§5) and confirming ownership.

**Byte contract:** this is a shared on-chain contract between the DIG store/node discovery path and
any wallet-side collateral/mirror tooling — the SAME morph derivation MUST be used on both sides for
hints to line up (see the superproject `SYSTEM.md` for the cross-repo discovery-hint contract). The
exact tree-hash derivation formula is implemented upstream in `datalayer_driver`/`chia_wallet_sdk`
and is outside this repository's verifiable surface; this crate's contract is that
`morph_store_launcher_id_for_collateral`/`morph_store_launcher_id_for_mirror` deterministically
derive the SAME namespace hash that the corresponding discovery-hint consumer expects — consumers
MUST call these functions rather than re-deriving the hash independently.

## 8. Memo layout

Mirror coins carry their mirror URLs as UTF-8-encoded strings in the coin's memo list, positioned
following the morphed store id used as the coin's discovery hint. A consumer reading a mirror coin's
memos MUST decode them as UTF-8 and MUST NOT assume a fixed memo count — the number of URL memos
varies with the number of mirror URLs supplied to `create_mirror`.

## 9. Network & fees

- `connect_random(network, ssl_cert_path, ssl_key_path) -> Peer` — connects to a random full-node
  peer on the given Chia network.
- `get_fee_estimate(&Peer, target_seconds: u32) -> u64` — estimates a mojo fee expected to confirm
  within `target_seconds`.
- All spend-building calls in this crate accept an explicit XCH `fee` and a `Vec<Coin>` of XCH
  inputs (`fee_coins`) to cover it; XCH fee coins travel alongside the `$DIG` CAT spends in the same
  spend bundle.
- `sign_coin_spends(spends, keys, use_agg_sig_me: bool) -> Signature` — signs the built spends.
  `use_agg_sig_me` MUST be `true` for any network other than mainnet (non-mainnet networks use the
  `AggSigMe` signing mode) and `false` for mainnet.
- `broadcast_spend_bundle(&Peer, SpendBundle) -> TransactionAck` — submits the signed bundle.
- Validation and hint-derivation in this crate use `MAINNET_CONSTANTS` by default (upstream); a
  consumer targeting a non-mainnet network MUST supply the matching `NetworkType` explicitly to
  `connect_random`/`get_fee_estimate`/`sign_coin_spends` — this crate does not infer network from
  context.

## 10. Errors

Fallible operations return upstream `WalletError` variants (e.g. `WalletError::UnknownCoin`,
`WalletError::PuzzleHashMismatch`, `WalletError::CoinIsAlreadySpent`) or network/parse errors
bubbled from `datalayer_driver`. A consumer MUST treat any `Err` from `from_coin_state`/`from_coin`
as "not a validated `$DIG` coin of the expected kind" — never assume success from partial data.

## 11. Conformance notes

- `tests/conformance.rs` is the crate's own conformance suite (§3) — it pins the `$DIG` asset id
  byte-exactly and guards the `DigCoin`/`DigCollateralCoin` re-exports against silent removal.
- This crate does not itself implement or test the CLVM puzzle reveals, the morphed-ID tree-hash
  formula, or the memo-encoding bytes at the wire level — those are upstream `datalayer_driver`/
  `chia_wallet_sdk` responsibilities. A consumer needing byte-level puzzle conformance tests should
  consult the upstream crates' own test suites and the docs.dig.net protocol pages for the `$DIG` CAT
  payment and DIG store discovery-hint contracts.
