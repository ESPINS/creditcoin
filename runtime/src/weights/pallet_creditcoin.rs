
//! Autogenerated weights for `pallet_creditcoin`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-03-24, STEPS: `50`, REPEAT: 30, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/creditcoin-node
// benchmark
// --chain
// dev
// --steps=50
// --repeat=30
// --pallet
// pallet_creditcoin
// --extrinsic=*
// --execution
// wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --output
// ./runtime/src/weights/pallet_creditcoin.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for `pallet_creditcoin`.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> pallet_creditcoin::WeightInfo for WeightInfo<T> {
	// Storage: Creditcoin Addresses (r:1 w:1)
	fn register_address(b: u32, e: u32, ) -> Weight {
		(18_924_000 as Weight)
			// Standard Error: 1_000
			.saturating_add((23_000 as Weight).saturating_mul(b as Weight))
			// Standard Error: 1_000
			.saturating_add((13_000 as Weight).saturating_mul(e as Weight))
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: Creditcoin LegacyWallets (r:1 w:1)
	// Storage: Creditcoin LegacyBalanceKeeper (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	fn claim_legacy_wallet() -> Weight {
		(73_100_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
}
