#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

// use codec::{Decode, Encode};
use sp_api::impl_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};

use sp_runtime::{
	create_runtime_str,
	// curve::PiecewiseLinear,
	generic, impl_opaque_keys,
	traits::{
		AccountIdLookup, BlakeTwo256, Block as BlockT, IdentifyAccount, NumberFor,
		OpaqueKeys, TypedGet, Verify,
	},
	transaction_validity::{TransactionPriority, TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, MultiSignature, Perbill, Percent, Permill,
};
use sp_staking::SessionIndex;
use sp_std::marker::PhantomData;

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

mod bag_thresholds;
use runtime_common::{
	auctions, paras_registrar, prod_or_fast, slots,
	SlowAdjustingFeeUpdate};
use runtime_parachains::{
	configuration as parachains_configuration,
	origin as parachains_origin, paras as parachains_paras,
	shared as parachains_shared,
};

use pallet_staking::UseValidatorsMap;
mod weights;

// A few exports that help ease life for downstream crates.
pub use frame_support::{
	assert_ok, construct_runtime, ord_parameter_types, parameter_types,
	traits::{
		ConstU128, ConstU32, ConstU64, ConstU8, Contains, EqualPrivilegeOnly,
		Everything, Get, InstanceFilter, KeyOwnerProofSystem, LockIdentifier, MapSuccess,
		OnInitialize, OriginTrait, Polling, PreimageRecipient, Randomness, SortedMembers,
		StorageInfo, TryMapSuccess, VoteTally, ContainsLengthBound,
	},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		IdentityFee, Weight,
		ConstantMultiplier,
	},
	PalletId, StorageValue,
};

use frame_election_provider_support::{onchain, SequentialPhragmen};

pub use frame_system::{Call as SystemCall, EnsureRoot, EnsureSigned,};
pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;

use pallet_grandpa::{
	fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList,
};
use pallet_transaction_payment::CurrencyAdapter;

pub mod governance;
use governance::{
	pallet_custom_origins, AuctionAdmin, GeneralAdmin,
	FellowshipCollectiveInstance, FellowshipReferendaInstance,
	FellowshipAdmin, LeaseAdmin, StakingAdmin, TreasurySpender};

pub const fn deposit(items: u32, bytes: u32) -> Balance {
	items as Balance * 2_000 * CENTS + (bytes as Balance) * 100 * MILLICENTS
}

pub type BlockNumber = u32;
pub type CurrencyToVote = frame_support::traits::U128CurrencyToVote;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use super::*;

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;

	impl_opaque_keys! {
		pub struct SessionKeys {
			pub aura: Aura,
			pub grandpa: Grandpa,
		}
	}
}

// To learn more about runtime versioning and what each of the following value means:
//   https://docs.substrate.io/v3/runtime/upgrades#runtime-versioning
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("governance2"),
	impl_name: create_runtime_str!("governance2"),
	authoring_version: 1,
	// The version of the runtime specification. A full node will not attempt to use its native
	//   runtime in substitute for the on-chain Wasm runtime unless all of `spec_name`,
	//   `spec_version`, and `authoring_version` are the same between Wasm and native.
	// This value is set to 100 to notify Polkadot-JS App (https://polkadot.js.org/apps) to use
	//   the compatible custom types.
	spec_version: 103,
	impl_version: 1,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 3,
	state_version: 3,
};

/// This determines the average expected block time that we are targeting.
/// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
/// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
/// up by `pallet_aura` to implement `fn slot_duration()`.
///
/// Change this to adjust the block time.
// pub const MILLISECS_PER_BLOCK: u64 = 6000;
pub const MILLISECS_PER_BLOCK: u64 = 3000;

// NOTE: Currently it is not possible to change the slot duration after the chain has started.
//       Attempting to do so will brick block production.
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

pub const UNIT: Balance = 1_000_000_000_000;
pub const UNITS: Balance = 1_000_000_000_000;
pub const DOLLARS: Balance = UNIT; // 1_000_000_000_000
pub const CENTS: Balance = DOLLARS / 100; // 10_000_000_000
pub const QUID: Balance = CENTS * 100;
pub const GRAND: Balance = QUID * 1_000;
pub const MILLICENTS: Balance = CENTS / 1_000; // 10_000_000
pub const EXISTENTIAL_DEPOSIT: Balance = 1 * CENTS;

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;
	pub const BlockHashCount: BlockNumber = 2400;
	/// We allow for 2 seconds of compute with a 6 second average block time.
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::with_sensible_defaults(
			WEIGHT_PER_SECOND.saturating_mul(2 as u64).set_proof_size(u64::MAX),
			NORMAL_DISPATCH_RATIO);
	pub BlockLength: frame_system::limits::BlockLength = frame_system::limits::BlockLength
		::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub const SS58Prefix: u8 = 42;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
	// /// The basic call filter to use in dispatchable.
	// type BaseCallFilter = frame_support::traits::Everything;
	// /// Block & extrinsics weights: base values and limits.
	// type BlockWeights = BlockWeights;
	// /// The maximum length of a block (in bytes).
	// type BlockLength = BlockLength;
	// /// The identifier used to distinguish between accounts.
	// type AccountId = AccountId;
	// /// The aggregated dispatch type that is available for extrinsics.
	// type RuntimeCall = RuntimeCall;
	// /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	// type Lookup = AccountIdLookup<AccountId, ()>;
	// /// The index type for storing how many extrinsics an account has signed.
	// type Index = Index;
	// /// The index type for blocks.
	// type BlockNumber = BlockNumber;
	// /// The type for hashing blocks and tries.
	// type Hash = Hash;
	// /// The hashing algorithm used.
	// type Hashing = BlakeTwo256;
	// /// The header type.
	// type Header = generic::Header<BlockNumber, BlakeTwo256>;
	// /// The ubiquitous event type.
	// type RuntimeEvent = RuntimeEvent;
	// /// The ubiquitous origin type.
	// // type Origin = Origin;
	// /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	// type BlockHashCount = BlockHashCount;
	// /// The weight of database operations that the runtime can invoke.
	// type DbWeight = RocksDbWeight;
	// /// Version of the runtime.
	// type Version = Version;
	// /// Converts a module to the index of the module in `construct_runtime!`.
	// ///
	// /// This type is being generated by `construct_runtime!`.
	// type PalletInfo = PalletInfo;
	// /// What to do if a new account is created.
	// type OnNewAccount = ();
	// /// What to do if an account is fully reaped from the system.
	// type OnKilledAccount = ();
	// /// The data to be stored in an account.
	// type AccountData = pallet_balances::AccountData<Balance>;
	// /// Weight information for the extrinsics of this pallet.
	// type SystemWeightInfo = ();
	// /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	// type SS58Prefix = SS58Prefix;
	// /// The set code logic, just the default since we're not a parachain.
	// type OnSetCode = ();
	// type MaxConsumers = frame_support::traits::ConstU32<16>;


	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = BlockWeights;
	type BlockLength = BlockLength;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	// type Index = Nonce;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = AccountIdLookup<AccountId, ()>;
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = RocksDbWeight;
	type Version = Version;
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	// type SystemWeightInfo = weights::frame_system::WeightInfo<Runtime>;
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;

}

impl pallet_randomness_collective_flip::Config for Runtime {}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = ConstU32<32>;
}

// DONE
parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	// FIXME: type OnTimestampSet = Babe;
	type OnTimestampSet = Aura;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = weights::pallet_timestamp::WeightInfo<Runtime>;
}

// DONE
parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * BlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;
	pub const NoPreimagePostponement: Option<u32> = Some(10);
}

impl pallet_scheduler::Config for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeEvent = RuntimeEvent;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = weights::pallet_scheduler::WeightInfo<Runtime>;
	// FIXME: type OriginPrivilegeCmp = OriginPrivilegeCmp;
	type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
	// type PreimageProvider = Preimage;
	// type NoPreimagePostponement = NoPreimagePostponement;
	type Preimages = Preimage;
}

// DONE
parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
	pub const MaxLocks: u32 = 50;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type WeightInfo = weights::pallet_balances::WeightInfo<Runtime>;
}
// DONE
parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 2000 * CENTS;
	pub const ProposalBondMaximum: Balance = 1 * GRAND;
	pub const SpendPeriod: BlockNumber = 6 * DAYS;
	pub const Burn: Permill = Permill::from_perthousand(2);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");

	pub const MaxApprovals: u32 = 100;
}

impl pallet_treasury::Config for Runtime {
	type PalletId = TreasuryPalletId;
	type Currency = Balances;
	type ApproveOrigin = EnsureRoot<AccountId>;
	type RejectOrigin = EnsureRoot<AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type ProposalBondMaximum = ProposalBondMaximum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	// FIXME: type BurnDestination = Society;
	type BurnDestination = ();
	type MaxApprovals = MaxApprovals;
	type WeightInfo = weights::pallet_treasury::WeightInfo<Runtime>;
	type SpendFunds = Bounties;
	type SpendOrigin = TreasurySpender;
}
// DONE
parameter_types! {
	pub const BountyDepositBase: Balance = 100 * CENTS;
	pub const BountyDepositPayoutDelay: BlockNumber = 4 * DAYS;
	pub const BountyUpdatePeriod: BlockNumber = 90 * DAYS;
	pub const MaximumReasonLength: u32 = 16384;
	pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
	pub const CuratorDepositMin: Balance = 10 * CENTS;
	pub const CuratorDepositMax: Balance = 500 * CENTS;
	pub const DataDepositPerByte: Balance = 1 * CENTS;
	pub const BountyValueMinimum: Balance = 200 * CENTS;
}

impl pallet_bounties::Config for Runtime {
	type BountyDepositBase = BountyDepositBase;
	type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
	type BountyUpdatePeriod = BountyUpdatePeriod;
	type CuratorDepositMultiplier = CuratorDepositMultiplier;
	type CuratorDepositMin = CuratorDepositMin;
	type CuratorDepositMax = CuratorDepositMax;
	type BountyValueMinimum = BountyValueMinimum;
	// FIXME: type ChildBountyManager = ChildBounties;
	type ChildBountyManager = ();
	type DataDepositPerByte = DataDepositPerByte;
	type RuntimeEvent = RuntimeEvent;
	type MaximumReasonLength = MaximumReasonLength;
	type WeightInfo = weights::pallet_bounties::WeightInfo<Runtime>;
}
// DONE
parameter_types! {
	pub const TipCountdown: BlockNumber = 1 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(20);
	pub const TipReportDepositBase: Balance = 100 * CENTS;
}

pub struct TestTippers;
impl SortedMembers<AccountId> for TestTippers {
	fn sorted_members() -> Vec<AccountId> {
		// TEN_TO_FOURTEEN.with(|v| v.borrow().clone())
		vec![]
	}
}
impl ContainsLengthBound for TestTippers {
	fn max_len() -> usize {
		0
	}
	fn min_len() -> usize {
		0
	}
}
impl pallet_tips::Config for Runtime {
	type MaximumReasonLength = MaximumReasonLength;
	type DataDepositPerByte = DataDepositPerByte;
	// (ksm) type Tippers = PhragmenElection;
	type Tippers = TestTippers;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_tips::WeightInfo<Runtime>;
}
// DONE
// impl pallet_authority_discovery::Config for Runtime {
// 	type MaxAuthorities = MaxAuthorities;
// }

// DONE
impl pallet_grandpa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	// type RuntimeCall = RuntimeCall;

	type KeyOwnerProof =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

	type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		GrandpaId,
	)>>::IdentificationTuple;

	// FIXME: type KeyOwnerProofSystem = Historical;
	type KeyOwnerProofSystem = ();

	// FIXME: type HandleEquivocation = pallet_grandpa::EquivocationHandler<
	// 	Self::KeyOwnerIdentification,
	// 	Offences,
	// 	ReportLongevity,
	// >;
	type HandleEquivocation = ();

	type WeightInfo = ();
	type MaxAuthorities = MaxAuthorities;
}

// DONE
impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
}

// DONE
impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
	RuntimeCall: From<C>,
{
	type Extrinsic = UncheckedExtrinsic;
	type OverarchingCall = RuntimeCall;
}
// DONE
pub struct OnChainSeqPhragmen;
impl onchain::Config for OnChainSeqPhragmen {
	type System = Runtime;
	type Solver = SequentialPhragmen<AccountId, runtime_common::elections::OnChainAccuracy>;
	type DataProvider = Staking;
	type WeightInfo = weights::frame_election_provider_support::WeightInfo<Runtime>;
}

// DONE
parameter_types! {
	pub const BagThresholds: &'static [u64] = &bag_thresholds::THRESHOLDS;
}

impl pallet_bags_list::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ScoreProvider = Staking;
	type WeightInfo = weights::pallet_bags_list::WeightInfo<Runtime>;
	type BagThresholds = BagThresholds;
	type Score = sp_npos_elections::VoteWeight;
}

// DONE
parameter_types! {
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
	/// This value increases the priority of `Operational` transactions by adding
	/// a "virtual tip" that's equal to the `OperationalFeeMultiplier * final_fee`.
	pub const OperationalFeeMultiplier: u8 = 5;
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	// (ksm) type OnChargeTransaction = CurrencyAdapter<Balances, DealWithFees<Self>>;
	type OnChargeTransaction = CurrencyAdapter<Balances, ()>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	// (ksm) type WeightToFee = WeightToFee;
	type WeightToFee = IdentityFee<Balance>;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
}

// DONE
parameter_types! {
	pub const PreimageMaxSize: u32 = 4096 * 1024;
	pub const PreimageBaseDeposit: Balance = deposit(2, 64);
	pub const PreimageByteDeposit: Balance = deposit(0, 1);
}

impl pallet_preimage::Config for Runtime {
	type WeightInfo = weights::pallet_preimage::WeightInfo<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>; // This might be too strong a requirenent?
	// type MaxSize = PreimageMaxSize;
	type BaseDeposit = PreimageBaseDeposit;
	type ByteDeposit = PreimageByteDeposit;
}

parameter_types! {
	pub const Period: u32 = 6 * HOURS;
	pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = AccountId;
	type ValidatorIdOf = pallet_staking::StashOf<Self>;
	// (ksm) type ShouldEndSession = Babe
	// (ksm) type NextSessionRotation = Babe
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	// (ksm) type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
	type SessionManager = ();
	type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = opaque::SessionKeys;
	type WeightInfo = weights::pallet_session::WeightInfo<Runtime>;
}

// DONE
impl pallet_session::historical::Config for Runtime {
	type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

parameter_types! {
	// Minimum 100 bytes/KSM deposited (1 CENT/byte)
	pub const BasicDeposit: Balance = 1000 * CENTS;       // 258 bytes on-chain
	pub const FieldDeposit: Balance = 250 * CENTS;        // 66 bytes on-chain
	pub const SubAccountDeposit: Balance = 200 * CENTS;   // 53 bytes on-chain
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxAdditionalFields: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
}
impl pallet_identity::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BasicDeposit = BasicDeposit;
	type FieldDeposit = FieldDeposit;
	type SubAccountDeposit = SubAccountDeposit;
	type MaxSubAccounts = MaxSubAccounts;
	type MaxAdditionalFields = MaxAdditionalFields;
	type MaxRegistrars = MaxRegistrars;
	type Slashed = Treasury;
	type ForceOrigin = GeneralAdmin;
	type RegistrarOrigin = GeneralAdmin;
	type WeightInfo = weights::pallet_identity::WeightInfo<Runtime>;
}

// DOING
// fn era_payout(
// 	total_staked: Balance,
// 	non_gilt_issuance: Balance,
// 	max_annual_inflation: Perquintill,
// 	period_fraction: Perquintill,
// 	auctioned_slots: u64,
// ) -> (Balance, Balance) {
// 	// use pallet_staking_reward_fn::compute_inflation;
// 	// use sp_arithmetic::traits::Saturating;

// 	// let min_annual_inflation = Perquintill::from_rational(25u64, 1000u64);
// 	// let delta_annual_inflation = max_annual_inflation.saturating_sub(min_annual_inflation);

// 	// // 30% reserved for up to 60 slots.
// 	// let auction_proportion = Perquintill::from_rational(auctioned_slots.min(60), 200u64);

// 	// // Therefore the ideal amount at stake (as a percentage of total issuance) is 75% less the amount that we expect
// 	// // to be taken up with auctions.
// 	// let ideal_stake = Perquintill::from_percent(75).saturating_sub(auction_proportion);

// 	// let stake = Perquintill::from_rational(total_staked, non_gilt_issuance);
// 	// let falloff = Perquintill::from_percent(5);
// 	// let adjustment = compute_inflation(stake, ideal_stake, falloff);
// 	// let staking_inflation =
// 	// 	min_annual_inflation.saturating_add(delta_annual_inflation * adjustment);

// 	// let max_payout = period_fraction * max_annual_inflation * non_gilt_issuance;
// 	// let staking_payout = (period_fraction * staking_inflation) * non_gilt_issuance;
// 	// let rest = max_payout.saturating_sub(staking_payout);

// 	// let other_issuance = non_gilt_issuance.saturating_sub(total_staked);
// 	// if total_staked > other_issuance {
// 	// 	let _cap_rest = Perquintill::from_rational(other_issuance, total_staked) * staking_payout;
// 	// 	// We don't do anything with this, but if we wanted to, we could introduce a cap on the treasury amount
// 	// 	// with: `rest = rest.min(cap_rest);`
// 	// }
// 	// (staking_payout, rest)

// 	(deposit(0, 32), deposit(0, 32))
// }

pub struct EraPayout;
impl pallet_staking::EraPayout<Balance> for EraPayout {
	fn era_payout(
		total_staked: Balance,
		_total_issuance: Balance,
		era_duration_millis: u64,
	) -> (Balance, Balance) {
		// TODO: #3011 Update with proper auctioned slots tracking.
		// This should be fine for the first year of parachains.
		// let auctioned_slots: u64 = auctions::Pallet::<Runtime>::auction_counter().into();
		// const MAX_ANNUAL_INFLATION: Perquintill = Perquintill::from_percent(10);
		// const MILLISECONDS_PER_YEAR: u64 = 1000 * 3600 * 24 * 36525 / 100;

		// era_payout(
		// 	total_staked,
		// 	Gilt::issuance().non_gilt,
		// 	MAX_ANNUAL_INFLATION,
		// 	Perquintill::from_rational(era_duration_millis, MILLISECONDS_PER_YEAR),
		// 	auctioned_slots,
		// )
		(deposit(0, 32), deposit(0, 32))
	}
}

parameter_types! {
	// Six sessions in an era (6 hours).
	pub const SessionsPerEra: SessionIndex = 6;
	// 28 eras for unbonding (7 days).
	pub const BondingDuration: sp_staking::EraIndex = 28;
	// 27 eras in which slashes can be cancelled (slightly less than 7 days).
	pub const SlashDeferDuration: sp_staking::EraIndex = 27;
	pub const MaxNominatorRewardedPerValidator: u32 = 256;
	pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
	// (ksm) pub const MaxNominations: u32 = <NposCompactSolution24 as NposSolution>::LIMIT as u32;
	pub const MaxNominations: u32 = 24;
}
impl pallet_staking::Config for Runtime {
	type MaxNominations = MaxNominations;
	type Currency = Balances;
	type CurrencyBalance = Balance;
	type UnixTime = Timestamp;
	type CurrencyToVote = CurrencyToVote;
	// (ksm) type ElectionProvider = ElectionProviderMultiPhase;
	type ElectionProvider = onchain::UnboundedExecution<OnChainSeqPhragmen>;
	type GenesisElectionProvider = onchain::UnboundedExecution<OnChainSeqPhragmen>;
	type RewardRemainder = Treasury;
	type RuntimeEvent = RuntimeEvent;
	type Slash = Treasury;
	type Reward = ();
	type SessionsPerEra = SessionsPerEra;
	type BondingDuration = BondingDuration;
	type SlashDeferDuration = SlashDeferDuration;
	// A majority of the council or root can cancel the slash.
	type SlashCancelOrigin = StakingAdmin;
	type SessionInterface = Self;
	type EraPayout = EraPayout;
	type NextNewSession = Session;
	type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
	type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
	type VoterList = VoterList;
	type TargetList = UseValidatorsMap<Self>;
	type MaxUnlockingChunks = frame_support::traits::ConstU32<32>;
	type HistoryDepth = frame_support::traits::ConstU32<84>;
	type BenchmarkingConfig = runtime_common::StakingBenchmarkingConfig;
	type OnStakerSlash = ();
	type WeightInfo = weights::pallet_staking::WeightInfo<Runtime>;
}

////////////////////////////////////////////////////////////////////////////////
// parameter_types! {
// 	pub CouncilMotionDuration: BlockNumber = prod_or_fast!(7 * DAYS, 2 * MINUTES, "DOT_MOTION_DURATION");
// 	pub const CouncilMaxProposals: u32 = 100;
// 	pub const CouncilMaxMembers: u32 = 100;
// }

parameter_types! {
	pub LaunchPeriod: BlockNumber = prod_or_fast!(28 * DAYS, 1, "DOT_LAUNCH_PERIOD");
	pub VotingPeriod: BlockNumber = prod_or_fast!(28 * DAYS, 1 * MINUTES, "DOT_VOTING_PERIOD");
	pub FastTrackVotingPeriod: BlockNumber = prod_or_fast!(3 * HOURS, 1 * MINUTES, "DOT_FAST_TRACK_VOTING_PERIOD");
	pub const MinimumDeposit: Balance = 100 * DOLLARS;
	pub EnactmentPeriod: BlockNumber = prod_or_fast!(28 * DAYS, 1, "DOT_ENACTMENT_PERIOD");
	pub CooloffPeriod: BlockNumber = prod_or_fast!(7 * DAYS, 1, "DOT_COOLOFF_PERIOD");
	pub const InstantAllowed: bool = true;
	pub const MaxVotes: u32 = 100;
	pub const MaxProposals: u32 = 100;
	pub const MaxDeposits: u32 = 100;
	pub const MaxBlacklisted: u32 = 10;
}

impl pallet_democracy::Config for Runtime {
	// type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type VoteLockingPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type MinimumDeposit = MinimumDeposit;
	/// A straight majority of the council can decide what their next motion is.
	// FIXME: Need check upstream settings
	type ExternalOrigin = EnsureRoot<AccountId>;
	/// A 60% super-majority can have the next scheduled referendum be a straight majority-carries
	/// vote.
	// FIXME: Need check upstream settings
	type ExternalMajorityOrigin = EnsureRoot<AccountId>;
	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	// FIXME: Need check upstream settings
	type ExternalDefaultOrigin = EnsureRoot<AccountId>;
	/// Two thirds of the technical committee can have an `ExternalMajority/ExternalDefault` vote
	/// be tabled immediately and with a shorter voting/enactment period.
	// FIXME: Need check upstream settings
	type FastTrackOrigin = EnsureRoot<AccountId>;
	// FIXME: Need check upstream settings
	type InstantOrigin = EnsureRoot<AccountId>;
	type InstantAllowed = InstantAllowed;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	// FIXME: Need check upstream settings
	type CancellationOrigin = EnsureRoot<AccountId>;
	// FIXME: Need check upstream settings
	type CancelProposalOrigin = EnsureRoot<AccountId>;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	// Any single technical committee member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cooloff period.
	// FIXME: Need check upstream settings
	type VetoOrigin = pallet_ranked_collective::EnsureMember<Runtime, FellowshipCollectiveInstance, 1>;
	type CooloffPeriod = CooloffPeriod;
	// type PreimageByteDeposit = PreimageByteDeposit;
	// FIXME: Need check upstream settings
	// type OperationalPreimageOrigin = pallet_ranked_collective::EnsureMember<Runtime, FellowshipCollectiveInstance, 1>;
	type Slash = Treasury;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type MaxVotes = MaxVotes;
	type WeightInfo = weights::pallet_democracy::WeightInfo<Runtime>;
	type MaxProposals = MaxProposals;

	type Preimages = Preimage;
	type MaxDeposits = ();
	type MaxBlacklisted = ();
}

parameter_types! {
	pub const CandidacyBond: Balance = 100 * DOLLARS;
	// 1 storage item created, key size is 32 bytes, value size is 16+16.
	pub const VotingBondBase: Balance = deposit(1, 64);
	// additional data per vote is 32 bytes (account id).
	pub const VotingBondFactor: Balance = deposit(0, 32);
	/// Weekly council elections; scaling up to monthly eventually.
	pub TermDuration: BlockNumber = prod_or_fast!(7 * DAYS, 2 * MINUTES, "DOT_TERM_DURATION");
	/// 13 members initially, to be increased to 23 eventually.

	pub const DesiredMembers: u32 = 13;
	pub const DesiredRunnersUp: u32 = 20;
	pub const PhragmenElectionPalletId: LockIdentifier = *b"phrelect";
}

////////////////////////////////////////////////////////////////////////////////
// Parachain Stuff
////////////////////////////////////////////////////////////////////////////////
parameter_types! {
	pub const ParaDeposit: Balance = 40 * UNIT;
}

impl parachains_origin::Config for Runtime {}

impl parachains_configuration::Config for Runtime {
	type WeightInfo = weights::runtime_parachains_configuration::WeightInfo<Runtime>;
}

impl parachains_shared::Config for Runtime {}

parameter_types! {
	pub const ParasUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
}

impl parachains_paras::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::runtime_parachains_paras::WeightInfo<Runtime>;
	type UnsignedPriority = ParasUnsignedPriority;
	// (ksm) type NextSessionRotation = Babe;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
}

parameter_types! {
	pub const FirstMessageFactorPercent: u64 = 100;
	pub const MaxAuthorities: u32 = 100_000;
}

/// Weight functions for `runtime_common::paras_registrar`.
pub struct ParasRegistrarWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> runtime_common::paras_registrar::WeightInfo
	for ParasRegistrarWeightInfo<T>
{
	// Storage: Registrar NextFreeParaId (r:1 w:1)
	// Storage: Registrar Paras (r:1 w:1)
	// Storage: Paras ParaLifecycles (r:1 w:0)
	fn reserve() -> Weight {
		Weight::from_ref_time(23_252_000 as u64)
			.saturating_add(T::DbWeight::get().reads(3 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Registrar Paras (r:1 w:1)
	// Storage: Paras ParaLifecycles (r:1 w:1)
	// Storage: Paras PvfActiveVoteMap (r:1 w:0)
	// Storage: Paras CodeByHash (r:1 w:1)
	// Storage: ParasShared CurrentSessionIndex (r:1 w:0)
	// Storage: Paras ActionsQueue (r:1 w:1)
	// Storage: Paras CodeByHashRefs (r:1 w:1)
	// Storage: Paras CurrentCodeHash (r:0 w:1)
	// Storage: Paras UpcomingParasGenesis (r:0 w:1)
	fn register() -> Weight {
		Weight::from_ref_time(8_592_152_000 as u64)
			.saturating_add(T::DbWeight::get().reads(7 as u64))
			.saturating_add(T::DbWeight::get().writes(7 as u64))
	}
	// Storage: Registrar Paras (r:1 w:1)
	// Storage: Paras ParaLifecycles (r:1 w:1)
	// Storage: Paras PvfActiveVoteMap (r:1 w:0)
	// Storage: Paras CodeByHash (r:1 w:1)
	// Storage: ParasShared CurrentSessionIndex (r:1 w:0)
	// Storage: Paras ActionsQueue (r:1 w:1)
	// Storage: Paras CodeByHashRefs (r:1 w:1)
	// Storage: Paras CurrentCodeHash (r:0 w:1)
	// Storage: Paras UpcomingParasGenesis (r:0 w:1)
	fn force_register() -> Weight {
		Weight::from_ref_time(8_532_677_000 as u64)
			.saturating_add(T::DbWeight::get().reads(7 as u64))
			.saturating_add(T::DbWeight::get().writes(7 as u64))
	}
	// Storage: Registrar Paras (r:1 w:1)
	// Storage: Paras ParaLifecycles (r:1 w:1)
	// Storage: Paras FutureCodeHash (r:1 w:0)
	// Storage: ParasShared CurrentSessionIndex (r:1 w:0)
	// Storage: Paras ActionsQueue (r:1 w:1)
	// Storage: Registrar PendingSwap (r:0 w:1)
	fn deregister() -> Weight {
		Weight::from_ref_time(45_897_000 as u64)
			.saturating_add(T::DbWeight::get().reads(5 as u64))
			.saturating_add(T::DbWeight::get().writes(4 as u64))
	}
	// Storage: Registrar Paras (r:1 w:0)
	// Storage: Paras ParaLifecycles (r:2 w:2)
	// Storage: Registrar PendingSwap (r:1 w:1)
	// Storage: ParasShared CurrentSessionIndex (r:1 w:0)
	// Storage: Paras ActionsQueue (r:1 w:1)
	// Storage: Crowdloan Funds (r:2 w:2)
	// Storage: Slots Leases (r:2 w:2)
	fn swap() -> Weight {
		Weight::from_ref_time(38_065_000 as u64)
			.saturating_add(T::DbWeight::get().reads(10 as u64))
			.saturating_add(T::DbWeight::get().writes(8 as u64))
	}
}

impl paras_registrar::Config for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type OnSwap = (Slots,);
	type ParaDeposit = ParaDeposit;
	type DataDepositPerByte = DataDepositPerByte;
	type WeightInfo = weights::runtime_common_paras_registrar::WeightInfo<Runtime>;
}

parameter_types! {
	// 6 weeks
	pub const WEEKS: BlockNumber = 7 * DAYS;
	pub LeasePeriod: BlockNumber = prod_or_fast!(42 * DAYS, 42 * DAYS, "KSM_LEASE_PERIOD");
}

/// Weight functions for `runtime_common::slots`.
pub struct SlotsWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> runtime_common::slots::WeightInfo for SlotsWeightInfo<T> {
	// Storage: Slots Leases (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	fn force_lease() -> Weight {
		Weight::from_ref_time(23_852_000 as u64)
			.saturating_add(T::DbWeight::get().reads(2 as u64))
			.saturating_add(T::DbWeight::get().writes(2 as u64))
	}
	// Storage: Paras Parachains (r:1 w:0)
	// Storage: Slots Leases (r:101 w:100)
	// Storage: Paras ParaLifecycles (r:101 w:101)
	// Storage: ParasShared CurrentSessionIndex (r:1 w:0)
	// Storage: Paras ActionsQueue (r:1 w:1)
	// Storage: Registrar Paras (r:100 w:100)
	fn manage_lease_period_start(c: u32, t: u32) -> Weight {
		Weight::from_ref_time(0 as u64)
			// Standard Error: 19_000
			.saturating_add(Weight::from_ref_time(7_061_000 as u64).saturating_mul(c as u64))
			// Standard Error: 19_000
			.saturating_add(Weight::from_ref_time(17_484_000 as u64).saturating_mul(t as u64))
			.saturating_add(T::DbWeight::get().reads(4 as u64))
			.saturating_add(T::DbWeight::get().reads((1 as u64).saturating_mul(c as u64)))
			.saturating_add(T::DbWeight::get().reads((3 as u64).saturating_mul(t as u64)))
			.saturating_add(T::DbWeight::get().writes(1 as u64))
			.saturating_add(T::DbWeight::get().writes((1 as u64).saturating_mul(c as u64)))
			.saturating_add(T::DbWeight::get().writes((3 as u64).saturating_mul(t as u64)))
	}
	// Storage: Slots Leases (r:1 w:1)
	// Storage: System Account (r:8 w:8)
	fn clear_all_leases() -> Weight {
		Weight::from_ref_time(93_598_000 as u64)
			.saturating_add(T::DbWeight::get().reads(9 as u64))
			.saturating_add(T::DbWeight::get().writes(9 as u64))
	}
	// Storage: Slots Leases (r:1 w:0)
	// Storage: Paras ParaLifecycles (r:1 w:1)
	// Storage: ParasShared CurrentSessionIndex (r:1 w:0)
	// Storage: Paras ActionsQueue (r:1 w:1)
	// Storage: Registrar Paras (r:1 w:1)
	fn trigger_onboard() -> Weight {
		Weight::from_ref_time(22_196_000 as u64)
			.saturating_add(T::DbWeight::get().reads(5 as u64))
			.saturating_add(T::DbWeight::get().writes(3 as u64))
	}
}

impl slots::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Registrar = Registrar;
	type LeasePeriod = LeasePeriod;
	type LeaseOffset = ();
	type ForceOrigin = LeaseAdmin;
	type WeightInfo = SlotsWeightInfo<Runtime>;
}

parameter_types! {
	// The average auction is 7 days long, so this will be 70% for ending period.
	// 5 Days = 72000 Blocks @ 6 sec per block
	pub const EndingPeriod: BlockNumber = 5 * DAYS;
	// ~ 1000 samples per day -> ~ 20 blocks per sample -> 2 minute samples
	pub const SampleLength: BlockNumber = 2 * MINUTES;
}

pub struct AuctionRandomness<T>(sp_std::marker::PhantomData<T>);
impl<T: frame_system::Config> Randomness<Hash, BlockNumber> for AuctionRandomness<T> {
	fn random(subject: &[u8]) -> (Hash, BlockNumber) {
		// FIXME
		(sp_core::hash::convert_hash(b"test"), 0)
	}
}
impl auctions::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Leaser = Slots;
	type Registrar = Registrar;
	type EndingPeriod = EndingPeriod;
	type SampleLength = SampleLength;
	// (ksm) type Randomness = pallet_babe::RandomnessFromOneEpochAgo<Runtime>;
	type Randomness = AuctionRandomness<Runtime>;
	type InitiateOrigin = AuctionAdmin;
	type WeightInfo = weights::runtime_common_auctions::WeightInfo<Runtime>;
}

impl pallet_utility::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = weights::pallet_utility::WeightInfo<Runtime>;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// Token related
		System: frame_system,

		// Babe must be before session.
		// Babe: pallet_babe,

		Timestamp: pallet_timestamp,
		Scheduler: pallet_scheduler,
		RandomnessCollectiveFlip: pallet_randomness_collective_flip,
		Aura: pallet_aura,
		Grandpa: pallet_grandpa,

		// Preimage: pallet_preimage::{Pallet, Call, Storage, Event<T>} = 32,
		Preimage: pallet_preimage,

		// Token related
		Balances: pallet_balances,
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>} = 33,
		Bounties: pallet_bounties,
		Tips: pallet_tips,
		Session: pallet_session,
		Historical: pallet_session::historical,
		VoterList: pallet_bags_list::{Pallet, Call, Storage, Event<T>} = 39,
		Staking: pallet_staking,

		// Governance
		Origins: pallet_custom_origins::{Origin},
		// Legacy:
		Democracy: pallet_democracy,
		// Council: pallet_collective::<Instance1>,
		// TechnicalCommittee: pallet_collective::<Instance2>,
		// PhragmenElection: pallet_elections_phragmen,
		// New:
		Whitelist: pallet_whitelist,
		Treasury: pallet_treasury,
		ConvictionVoting: pallet_conviction_voting::{Pallet, Call, Storage, Event<T>} = 20,
		Referenda: pallet_referenda,
		FellowshipCollective: pallet_ranked_collective::<Instance1>,
		FellowshipReferenda: pallet_referenda::<Instance2>,

		Utility: pallet_utility::{Pallet, Call, Event} = 24,
		Identity: pallet_identity::{Pallet, Call, Storage, Event<T>} = 25,

		// Parachains pallets. Start indices at 50 to leave room.
		ParachainsOrigin: parachains_origin::{Pallet, Origin} = 50,
		Configuration: parachains_configuration::{Pallet, Call, Storage, Config<T>} = 51,
		ParasShared: parachains_shared::{Pallet, Call, Storage} = 52,
		Paras: parachains_paras::{Pallet, Call, Storage, Event, Config} = 56,

		// Parachain Onboarding Pallets. Start indices at 70 to leave room.
		Registrar: paras_registrar::{Pallet,Call, Storage, Event<T>} = 70,
		Slots: slots::{Pallet, Call, Storage, Event<T>} = 71,
		Auctions: auctions::{Pallet, Call, Storage, Event<T>} = 72,

		Sudo: pallet_sudo,
	}
);

/// The address format for describing accounts.
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate frame_benchmarking;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	define_benchmarks!(
		[frame_benchmarking, BaselineBench::<Runtime>]
			[frame_system, SystemBench::<Runtime>]
			[pallet_balances, Balances]
			[pallet_timestamp, Timestamp]
	);
}

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block);
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			Aura::authorities().into_inner()
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			opaque::SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}

		fn current_set_id() -> fg_primitives::SetId {
			Grandpa::current_set_id()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			_equivocation_proof: fg_primitives::EquivocationProof<
					<Block as BlockT>::Hash,
				NumberFor<Block>,
				>,
			_key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			None
		}

		fn generate_key_ownership_proof(
			_set_id: fg_primitives::SetId,
			_authority_id: GrandpaId,
		) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
			// NOTE: this is the only implementation possible since we've
			// defined our key owner proof type as a bottom type (i.e. a type
			// with no values).
			None
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::{baseline, Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;
			use frame_system_benchmarking::Pallet as SystemBench;
			use baseline::Pallet as BaselineBench;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();

			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{baseline, Benchmarking, BenchmarkBatch, TrackedStorageKey};

			use frame_system_benchmarking::Pallet as SystemBench;
			use baseline::Pallet as BaselineBench;

			impl frame_system_benchmarking::Config for Runtime {}
			impl baseline::Config for Runtime {}

			let whitelist: Vec<TrackedStorageKey> = vec![
				// Block Number
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
				// RuntimeEvent Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
			];

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			Ok(batches)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade() -> (Weight, Weight) {
			// NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
			// have a backtrace here. If any of the pre/post migration checks fail, we shall stop
			// right here and right now.
			let weight = Executive::try_runtime_upgrade().unwrap();
			(weight, BlockWeights::get().max_block)
		}

		fn execute_block_no_check(block: Block) -> Weight {
			Executive::execute_block_no_check(block)
		}
	}
}
