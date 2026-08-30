//! This crate must not vend a morph that lands in `dig-mirror-coin`'s namespace.
//!
//! `dig-mirror-coin` is the canonical owner of DIG mirror collateral. It hints a mirror coin under
//! `morph(store + root + owner + epoch)`, tagged `DIG_STORE_MIRROR_COLLATERAL`. The upstream
//! `datalayer_driver` this crate re-exports once carried a **two-term** ancestor of that morph,
//! `morph(store + epoch)`, under the *identical* tag; `datalayer_driver` 6.0.0 removed it and its
//! `create_mirror` mint path, leaving `dig-mirror-coin` the sole producer in that namespace.
//!
//! ## Why removal was the only available remedy, asserted rather than asserted-about
//!
//! It is tempting to read the old arrangement as two schemes that merely shared a label, and to
//! conclude that renaming one would have been enough. It would not have been, and the tests below
//! are here so that conclusion is checkable rather than remembered: both morphs hash an **additive**
//! sum, so the two extra terms of the canonical form are absorbed instead of separating it from the
//! two-term form. The two were one namespace, and a namespace collision is not fixed by a label.
//!
//! The retired morph is therefore **transcribed locally** here, from the formula recorded in
//! `SPEC.md` §7.1, rather than called. Transcribing it keeps the evidence executable after the
//! upstream removal, and it keeps this file honest about what it is testing: the danger is the
//! *shape* of that derivation, not one crate's copy of it. If anyone reintroduces an additive
//! two-term morph under the mirror tag — here or anywhere — these tests say what it would cost.
//!
//! Everything asserted below is a comparison of computed 32-byte values. Nothing here asserts on the
//! presence or absence of a symbol.

use clvm_utils::ToTreeHash;
use num_bigint::BigInt;

/// The canonical mirror namespace tag, transcribed from `dig-mirror-coin`'s SPEC.
const MIRROR_NAMESPACE: &str = "DIG_STORE_MIRROR_COLLATERAL";

/// The store-collateral namespace tag, transcribed from `SPEC.md` §7.
const STORE_COLLATERAL_NAMESPACE: &str = "DIG_STORE_COLLATERAL";

fn store_id() -> [u8; 32] {
    [0x11; 32]
}

fn root() -> [u8; 32] {
    [0x22; 32]
}

fn owner() -> [u8; 32] {
    [0x33; 32]
}

/// The canonical four-term mirror hint, as `dig-mirror-coin` computes it.
fn canonical_mirror_hint(
    store: [u8; 32],
    root: [u8; 32],
    owner: [u8; 32],
    epoch: &BigInt,
) -> [u8; 32] {
    dig_mirror_coin::mirror_hint(
        chia_protocol::Bytes32::new(store),
        chia_protocol::Bytes32::new(root),
        chia_protocol::Bytes32::new(owner),
        epoch,
    )
    .to_bytes()
}

/// The store-collateral hint this crate actually vends, via its `datalayer_driver` re-export.
fn vended_store_collateral_hint(store: [u8; 32]) -> [u8; 32] {
    dig_collateral_coin::DigCollateralCoin::morph_store_launcher_id_for_collateral(
        datalayer_driver::Bytes32::new(store),
    )
    .to_bytes()
}

/// The store-collateral hint as `SPEC.md` §7 *describes* it, transcribed from that prose rather than
/// read out of the upstream implementation.
///
/// Reading the upstream code to build this would only prove the code agrees with itself. Written
/// from the spec text, it proves the spec's byte contract describes what the crate really forwards.
fn store_collateral_hint_per_spec(store: [u8; 32]) -> [u8; 32] {
    let hash: chia_protocol::Bytes32 = (
        chia_protocol::Bytes32::new(store),
        STORE_COLLATERAL_NAMESPACE,
    )
        .tree_hash()
        .into();
    hash.to_bytes()
}

/// The retired two-term mirror morph, transcribed from `SPEC.md` §7.1.
///
/// This crate does not vend this and `datalayer_driver` 6.0.0 no longer defines it. It exists here
/// only so the reason for that removal stays executable.
fn retired_two_term_hint(store: [u8; 32], epoch: &BigInt) -> [u8; 32] {
    let sum = BigInt::from_signed_bytes_be(&store) + epoch;
    let hash: chia_protocol::Bytes32 = (sum, MIRROR_NAMESPACE).tree_hash().into();
    hash.to_bytes()
}

/// The canonical namespace tag has not moved. `dig-mirror-coin` documents it as a wire constant:
/// changing it re-homes every mirror coin and orphans every coin already on chain. If this fails,
/// nothing else in this file means what it says.
#[test]
fn the_canonical_mirror_namespace_tag_is_unchanged() {
    assert_eq!(dig_mirror_coin::MIRROR_NAMESPACE, MIRROR_NAMESPACE);
}

/// A truthful control: the canonical morph does distinguish advertisements that differ only in the
/// epoch. Without it, a file in which things collide would be indistinguishable from a broken
/// harness returning a constant.
#[test]
fn the_canonical_morph_separates_two_epochs_of_one_advertisement() {
    let a = canonical_mirror_hint(store_id(), root(), owner(), &BigInt::from(7));
    let b = canonical_mirror_hint(store_id(), root(), owner(), &BigInt::from(8));

    assert_ne!(
        a, b,
        "the canonical morph is not varying with its epoch; the harness is wrong, not the code"
    );
}

/// `SPEC.md` §7's byte contract: this crate forwards the upstream store-collateral derivation
/// unchanged. Asserted against a transcription of the spec's own words, so a silent upstream change
/// to the derivation is caught here rather than discovered by a hint that finds nothing.
#[test]
fn the_vended_store_collateral_hint_matches_the_spec_derivation() {
    assert_eq!(
        vended_store_collateral_hint(store_id()),
        store_collateral_hint_per_spec(store_id()),
        "the store-collateral morph this crate re-exports no longer matches the derivation SPEC.md \
         §7 promises consumers"
    );
}

/// The property that survives the removal: a store-collateral coin is not discoverable as a mirror
/// coin. The two live namespaces are genuinely distinct, and unlike the retired morph they are
/// distinguished by their tags rather than only by their arithmetic.
#[test]
fn the_store_collateral_namespace_is_disjoint_from_the_mirror_namespace() {
    // Epoch zero is the sharpest case: it is where the canonical morph's offset contributes nothing,
    // so any collision between the two namespaces would surface here first.
    let mirror = canonical_mirror_hint(store_id(), root(), owner(), &BigInt::from(0));

    assert_ne!(
        vended_store_collateral_hint(store_id()),
        mirror,
        "a store-collateral coin lands on a mirror hint: the two namespaces have collapsed"
    );
}

/// The degenerate overlap that made the retired morph unfixable by renaming: the canonical form's
/// extra terms are *added*, so an advertisement whose root and owner are zero reduces to exactly the
/// two-term sum, under exactly the same tag.
///
/// This is the belief a reader is most likely to hold on seeing a two-argument and a four-argument
/// derivation — that differing arity separates them. It does not.
#[test]
fn a_zero_root_and_owner_advertisement_lands_on_the_retired_two_term_hint() {
    let epoch = BigInt::from(7);

    assert_eq!(
        canonical_mirror_hint(store_id(), [0u8; 32], [0u8; 32], &epoch),
        retired_two_term_hint(store_id(), &epoch),
        "the retired two-term morph is expected to collide with the canonical one here; if it no \
         longer does, the canonical derivation has changed and SPEC.md §7.1 needs rewriting"
    );
}

/// The reachable overlap, and the one that decided the removal. The epoch is an unbounded integer
/// chosen by whoever builds the coin, so its author can solve
/// `e' = store + epoch - store' - root' - owner'` and land a four-term advertisement bonding their
/// OWN store and root exactly on a hint derived by the two-term form.
///
/// This is a second actor at a second position rather than a restatement of the degenerate case:
/// every term here is attacker-chosen and distinct from the victim's, and only the epoch is solved.
/// A remedy that merely stopped the zero-term reduction would have left this standing — which is why
/// the two-term morph was removed rather than narrowed.
#[test]
fn a_solved_epoch_lands_an_attackers_advertisement_on_a_retired_two_term_hint() {
    let victim_epoch = BigInt::from(42);
    let victim = retired_two_term_hint(store_id(), &victim_epoch);

    let their_store = [0x77u8; 32];
    let their_root = [0x88u8; 32];
    let their_owner = [0x99u8; 32];

    let solved_epoch = BigInt::from_signed_bytes_be(&store_id()) + &victim_epoch
        - BigInt::from_signed_bytes_be(&their_store)
        - BigInt::from_signed_bytes_be(&their_root)
        - BigInt::from_signed_bytes_be(&their_owner);

    assert_eq!(
        canonical_mirror_hint(their_store, their_root, their_owner, &solved_epoch),
        victim,
        "the solved-epoch collision is expected to hold; if it no longer does, the canonical \
         derivation has changed and SPEC.md §7.1 needs rewriting"
    );

    // The control that keeps the assertion above from passing for a trivial reason: without the
    // solved epoch, the attacker's advertisement is nowhere near the victim's hint.
    assert_ne!(
        canonical_mirror_hint(their_store, their_root, their_owner, &victim_epoch),
        victim,
        "the attacker's advertisement collides without solving for the epoch; the fixture is not \
         exercising the solve"
    );
}

/// The transcription above must reproduce the function that was actually removed, or every claim
/// this file makes about the retired morph is a self-consistent fiction.
///
/// These 32 bytes were produced by the real `datalayer_driver` 3.0.0
/// `DigCollateralCoin::morph_store_launcher_id_for_mirror(Bytes32::new([0x11; 32]), &42)` before its
/// removal, and are pinned here so the transcription is checked against the original rather than
/// against itself.
#[test]
fn the_transcribed_retired_morph_reproduces_the_removed_upstream_function() {
    const OBSERVED_UPSTREAM: [u8; 32] = [
        237, 188, 218, 225, 143, 199, 74, 239, 78, 46, 251, 142, 176, 208, 97, 142, 60, 115, 57,
        79, 94, 180, 1, 149, 5, 5, 233, 235, 180, 226, 103, 20,
    ];

    assert_eq!(
        retired_two_term_hint(store_id(), &BigInt::from(42)),
        OBSERVED_UPSTREAM,
        "the local transcription of the retired two-term morph no longer matches the bytes the \
         removed upstream function produced; the collision evidence in this file is void until it \
         does"
    );
}
