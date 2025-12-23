dig-collateral-coin

Thin, ergonomic re-export crate that surfaces the minimal interfaces you need to work with $DIG CAT coins and $DIG collateral/mirror coins on Chia using the upstream `datalayer_driver` and `chia_wallet_sdk` libraries.

This crate intentionally exposes only a small surface area while remaining fully compatible with the broader upstream ecosystem.

- Crate status: thin wrapper around upstream libraries
- License: see upstream projects
- Minimum supported Rust: follow upstream requirements


## Key crate concepts

- DIG CAT ($DIG)
  - A Chia Asset Token (CAT) representing the $DIG token.
  - The `DigCoin` helper validates a coin is a bona fide $DIG CAT and exposes the parsed `Cat` driver so you can spend it.

- Collateral and Mirror coins
  - Collateral coins are $DIG locked by a `P2Parent` puzzle for a given store. They secure a DIG store and can be reclaimed by the creator.
  - Mirror coins are $DIG locked similarly, but tagged for a specific epoch and with mirror URLs in memos for replication.
  - `DigCollateralCoin` wraps the logic to detect, construct, and spend these coins.

- Namespacing via morphed IDs and memos
  - Store launcher IDs are morphed into distinct namespaces (tree hashes) to derive hints/memos used for discovery on-chain:
    - `morph_store_launcher_id_for_collateral(store_id)` → hint for collateral coins
    - `morph_store_launcher_id_for_mirror(store_id, epoch)` → hint for mirror coins
  - Mirror coins store additional memos (UTF-8 URLs) following the morphed store ID.

- Standard layer (synthetic key)
  - Collateral coins are controlled by a standard P2 (synthetic) key layer. You must use the same synthetic public key to create and later reclaim collateral.

- Network and fees
  - Network operations are executed via a `Peer` connection to a Chia full node.
  - Fees are paid in XCH using standard coins included alongside CAT spends.


## Usage Guide

This crate re-exports the minimum types and helpers from `datalayer_driver` that you need in your application, plus `num_bigint` for epoch calculations.

Add to Cargo.toml (replace version with what you use):

```
[dependencies]
dig-collateral-coin = { path = "." }
```

Import what you need:

```
use dig_collateral_coin::{
    DigCoin, DigCollateralCoin, Bytes32, Peer, PublicKey, SpendBundle,
    connect_random, get_fee_estimate, sign_coin_spends, broadcast_spend_bundle,
};
use dig_collateral_coin::num_bigint::BigInt;
```

### 1) Compute the DIG collateral puzzle hash for your wallet (optional helper)

If you need the CAT outer puzzle hash for a wallet puzzle hash (owner PH):

```
let wallet_ph: Bytes32 = /* your wallet owner PH */;
let dig_cat_ph = DigCoin::puzzle_hash(wallet_ph);
```

### 2) Validate a coin is a $DIG CAT

Given a `CoinState` fetched from the node, prove it is a $DIG CAT of the expected asset id:

```
async fn prove_dig_coin(peer: &Peer, coin_state: &chia::protocol::CoinState) -> Result<DigCoin, anyhow::Error> {
    let dig = DigCoin::from_coin_state(peer, coin_state).await?;
    Ok(dig)
}
```

### 3) Create collateral for a store

- Select sufficient $DIG CAT inputs and XCH fee coins with your wallet.
- Use your wallet synthetic public key to control the created collateral coin.

```
async fn create_store_collateral(
    network: datalayer_driver::NetworkType,
    ssl_cert: &str,
    ssl_key: &str,
    public_synthetic_key: PublicKey,
    dig_inputs: Vec<DigCoin>,
    store_id: Bytes32,
) -> anyhow::Result<datalayer_driver::TransactionAck> {
    let peer = connect_random(network, ssl_cert, ssl_key).await?;

    // Estimate or set a fee in mojos
    let fee = get_fee_estimate(&peer, 60).await?;

    // Choose an amount (application-specific). Example: 1_000_000 DIG mojos
    let amount: u64 = 1_000_000;

    // Pick XCH coins to cover the fee
    let xch_fee_coins = /* your wallet selects fee coins */ Vec::<chia::protocol::Coin>::new();

    let spends = DigCollateralCoin::create_collateral(
        dig_inputs,
        amount,
        store_id,
        public_synthetic_key,
        xch_fee_coins,
        fee,
    )?;

    // Sign with your synthetic private key
    let synthetic_sk = /* your wallet synthetic private key */ datalayer_driver::PrivateKey::from(&[0u8; 32]);
    let sig = sign_coin_spends(&spends, &[synthetic_sk], network != datalayer_driver::NetworkType::Mainnet)?;
    let bundle = SpendBundle::new(spends, sig);

    let ack = broadcast_spend_bundle(&peer, bundle).await?;
    Ok(ack)
}
```

### 4) Discover existing collateral for a store you own

Search by the morphed hint and validate the coin belongs to your wallet by checking the parent inner puzzle hash.

```
async fn find_my_highest_collateral(
    network: datalayer_driver::NetworkType,
    ssl_cert: &str,
    ssl_key: &str,
    wallet_owner_ph: Bytes32,
    store_id: Bytes32,
    min_amount: u64,
) -> anyhow::Result<Option<DigCollateralCoin>> {
    use std::cmp::Reverse;

    let peer = connect_random(network, ssl_cert, ssl_key).await?;

    let hint = DigCollateralCoin::morph_store_launcher_id_for_collateral(store_id);
    let mut states = datalayer_driver::get_unspent_coin_states_by_hint(&peer, hint, network)
        .await?
        .coin_states
        .into_iter()
        .filter(|cs| cs.coin.amount >= min_amount)
        .collect::<Vec<_>>();

    states.sort_unstable_by_key(|cs| Reverse(cs.coin.amount));

    for cs in states {
        if let Ok(collateral) = DigCollateralCoin::from_coin_state(&peer, cs).await {
            if collateral.proof().parent_inner_puzzle_hash == wallet_owner_ph {
                return Ok(Some(collateral));
            }
        }
    }

    Ok(None)
}
```

### 5) Create mirror coins

Mirror coins include the morphed store id and a list of mirror URLs in the memos; they are also tied to an epoch value.

```
async fn create_store_mirror(
    network: datalayer_driver::NetworkType,
    ssl_cert: &str,
    ssl_key: &str,
    public_synthetic_key: PublicKey,
    dig_inputs: Vec<DigCoin>,
    store_id: Bytes32,
    epoch: num_bigint::BigInt,
    mirror_urls: Vec<String>,
) -> anyhow::Result<datalayer_driver::TransactionAck> {
    let peer = connect_random(network, ssl_cert, ssl_key).await?;
    let fee = get_fee_estimate(&peer, 60).await?;

    let amount: u64 = 1_000_000; // example
    let xch_fee_coins = /* your wallet selects fee coins */ Vec::<chia::protocol::Coin>::new();

    let spends = DigCollateralCoin::create_mirror(
        dig_inputs,
        amount,
        store_id,
        mirror_urls,
        epoch,
        public_synthetic_key,
        xch_fee_coins,
        fee,
    )?;

    let synthetic_sk = /* your wallet synthetic private key */ datalayer_driver::PrivateKey::from(&[0u8; 32]);
    let sig = sign_coin_spends(&spends, &[synthetic_sk], network != datalayer_driver::NetworkType::Mainnet)?;
    let bundle = SpendBundle::new(spends, sig);

    Ok(broadcast_spend_bundle(&peer, bundle).await?)
}
```

### 6) Reclaim collateral (or mirror) coins you own

Spend the P2Parent-locked coin back to your standard P2 synthetic key.

```
async fn reclaim(
    network: datalayer_driver::NetworkType,
    ssl_cert: &str,
    ssl_key: &str,
    public_synthetic_key: PublicKey,
    collateral: DigCollateralCoin,
) -> anyhow::Result<(datalayer_driver::TransactionAck, Bytes32)> {
    let peer = connect_random(network, ssl_cert, ssl_key).await?;
    let fee = get_fee_estimate(&peer, 60).await?;

    let xch_fee_coins = /* your wallet selects fee coins */ Vec::<chia::protocol::Coin>::new();

    let spends = collateral.spend(public_synthetic_key, xch_fee_coins, fee)?;

    let synthetic_sk = /* your wallet synthetic private key */ datalayer_driver::PrivateKey::from(&[0u8; 32]);
    let sig = sign_coin_spends(&spends, &[synthetic_sk], network != datalayer_driver::NetworkType::Mainnet)?;
    let bundle = SpendBundle::new(spends, sig);

    let ack = broadcast_spend_bundle(&peer, bundle).await?;
    Ok((ack, collateral.coin().coin_id()))
}
```


## Comprehensive reference guide

This crate re-exports the following items from upstream so you can build apps without pulling in the entire upstream API surface:

- Types and helpers
  - `DigCoin`
    - `fn cat(&self) -> Cat` — returns the underlying CAT driver (from `chia_wallet_sdk`).
    - `fn puzzle_hash(wallet_puzzle_hash: Bytes32) -> Bytes32` — compute the $DIG CAT puzzle hash for a given wallet owner PH.
    - `async fn from_coin_state(peer: &Peer, coin_state: &CoinState) -> Result<Self, WalletError>` — validate and instantiate from a coin state.
    - `async fn from_coin(peer: &Peer, coin: &Coin, created_height: u32) -> Result<Self, WalletError>` — low-level constructor when you have `Coin` + height.

  - `DigCollateralCoin`
    - `fn coin(&self) -> Coin` — the underlying coin.
    - `fn proof(&self) -> LineageProof` — lineage proof used to verify control and spendability.
    - `async fn from_coin_state(peer: &Peer, coin_state: CoinState) -> Result<Self, WalletError>` — instantiate and verify a P2Parent $DIG collateral/mirror coin.
    - `fn create_collateral(dig_coins, amount, store_id, synthetic_key, fee_coins, fee) -> Result<Vec<CoinSpend>, WalletError>` — build spends to create a collateral coin.
    - `fn create_mirror(dig_coins, amount, store_id, mirror_urls, epoch, synthetic_key, fee_coins, fee) -> Result<Vec<CoinSpend>, WalletError>` — build spends to create mirror coins with memos.
    - `fn spend(&self, synthetic_key, fee_coins, fee) -> Result<Vec<CoinSpend>, WalletError>` — spend a collateral/mirror coin you control back to your P2 synthetic key.
    - `fn morph_store_launcher_id_for_collateral(store_id) -> Bytes32` — derive the collateral hint namespace.
    - `fn morph_store_launcher_id_for_mirror(store_id, epoch: &BigInt) -> Bytes32` — derive the mirror hint namespace.

  - Networking and tx helpers
    - `connect_random(network, ssl_cert_path, ssl_key_path) -> Peer` — connect to a random peer for the given network.
    - `get_fee_estimate(&Peer, target_seconds) -> u64` — estimate fee.
    - `sign_coin_spends(spends, keys, use_agg_sig_me) -> Signature` — sign spends.
    - `broadcast_spend_bundle(&Peer, SpendBundle) -> TransactionAck` — submit the transaction.

  - Commonly used primitives re-exported for convenience
    - `Bytes32`, `Peer`, `CoinState`, `PublicKey`, `SpendBundle`
    - `num_bigint` (crate re-export) for `BigInt` epochs

Notes and guarantees:

- Validation during construction
  - `DigCoin::from_coin_state` proves the coin is a $DIG CAT by parsing the parent spend and ensuring the asset id matches `DIG_ASSET_ID` and lineage is present.
  - `DigCollateralCoin::from_coin_state` verifies the coin is unspent and locked by the expected $DIG P2Parent puzzle and captures memos for store/mirror semantics.

- Spending model
  - Spends are prepared using `chia_wallet_sdk::driver` constructs (Actions, Spends, Conditions) and must be signed with your synthetic key.
  - Fees are attached with XCH inputs and `AssertConcurrentSpend` to safely couple fee spends with CAT spends.

- Network constants
  - Uses `MAINNET_CONSTANTS` by default when deriving and validating on-chain data; ensure you supply the right network when connecting and estimating fees.

- Errors
  - Functions return domain errors like `WalletError::UnknownCoin`, `WalletError::PuzzleHashMismatch`, `WalletError::CoinIsAlreadySpent`, or networking/parse errors bubbled from upstream.


## End-to-end examples

These end-to-end flows are typical in higher-level apps that use this crate. Pseudocode derived from real usage:

- Collateralize a store: select DIG + XCH, build spends via `create_collateral`, sign, broadcast.
- Check if a store is collateralized: fetch by morphed hint, validate ownership, compare against required amount.
- Create and enumerate mirror coins: derive mirror namespace with epoch, include URLs as memos, separate owned vs external.
- Reclaim collateral: spend the P2Parent coin back to your synthetic P2 and receive unlocked $DIG.

Refer to the Usage Guide for code snippets you can adapt directly.


## Troubleshooting

- Invalid coin errors when proving $DIG:
  - Ensure the coin really is a $DIG CAT; `from_coin_state` parses and checks lineage and asset id.
- Collateral coin controlled by another wallet:
  - `spend()` validates the `parent_inner_puzzle_hash` matches the hash of your synthetic P2 layer; use the correct synthetic key pair.
- No coins found by hint:
  - Verify you derived the correct morphed store id and used the correct epoch for mirror coins.
- Fee or mempool rejection:
  - Increase fee via `get_fee_estimate` or set a manual fee; ensure fee XCH inputs are included.


## Safety and operational notes

- Keep your synthetic private key secure; it authorizes creation and reclamation.
- Always test on testnet/regtest before mainnet. The signing helper exposes a flag to use AggSigMe outside mainnet.
- Network operations depend on a healthy full node; transient errors can occur.
