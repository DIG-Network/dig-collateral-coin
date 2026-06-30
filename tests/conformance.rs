//! Protocol-conformance guards for the `$DIG` CAT constants this crate vends.
//!
//! `dig-collateral-coin` is a thin re-export shim over `datalayer_driver`; it owns no
//! constants of its own. Its in-scope (Protocol spec Layers 0–5) surface is the **DIG CAT
//! payment** asset id, which the re-exported `DigCoin` / `DigCollateralCoin` validate against.
//! These tests pin the byte-exact mainnet TAIL hash from the normative spec
//! (`docs.dig.net/docs/protocol/dig-cat-payment.md`) so an upstream `datalayer-driver` bump
//! cannot silently change the asset id this crate surfaces.
//!
//! Spec: `DIG_ASSET_ID` (TAIL hash, mainnet) =
//! `a406d3a9de984d03c9591c10d917593b434d5263cabe2b42f6b367df16832f81`.

/// The byte-exact `$DIG` mainnet TAIL hash, copied from the normative protocol spec.
const SPEC_DIG_ASSET_ID: [u8; 32] = [
    0xa4, 0x06, 0xd3, 0xa9, 0xde, 0x98, 0x4d, 0x03, 0xc9, 0x59, 0x1c, 0x10, 0xd9, 0x17, 0x59, 0x3b,
    0x43, 0x4d, 0x52, 0x63, 0xca, 0xbe, 0x2b, 0x42, 0xf6, 0xb3, 0x67, 0xdf, 0x16, 0x83, 0x2f, 0x81,
];

/// The `$DIG` asset id the crate vends (via its `datalayer_driver` dependency, the source of the
/// `DigCoin`/`DigCollateralCoin` validation this crate re-exports) MUST equal the spec TAIL hash.
#[test]
fn dig_asset_id_matches_protocol_spec() {
    let vended: [u8; 32] = datalayer_driver::wallet::DIG_ASSET_ID.to_bytes();
    assert_eq!(
        vended, SPEC_DIG_ASSET_ID,
        "DIG_ASSET_ID surfaced by this crate drifted from the protocol spec TAIL hash \
         a406d3a9de984d03c9591c10d917593b434d5263cabe2b42f6b367df16832f81"
    );
}

/// The crate must continue to re-export the `$DIG`-validating coin types it documents, so a
/// consumer can prove a coin is a bona fide `$DIG` CAT. Pure compile-time surface check.
#[test]
fn vends_dig_cat_coin_types() {
    // Referencing the items is enough; a missing/renamed re-export fails to compile.
    let _coin: Option<dig_collateral_coin::DigCoin> = None;
    let _collateral: Option<dig_collateral_coin::DigCollateralCoin> = None;
}
