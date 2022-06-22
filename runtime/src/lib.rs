#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use codec::{Decode, Encode, MaxEncodedLen};
use sp_api::impl_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
// use primitives::v2::{
// 	AccountIndex, CandidateEvent, CandidateHash,
// 	CommittedCandidateReceipt, CoreState, DisputeState, GroupRotationInfo,  Id as ParaId,
// 	InboundDownwardMessage, InboundHrmpMessage, Moment, Nonce, OccupiedCoreAssumption,
// 	PersistedValidationData, PvfCheckStatement, ScrapedOnChainVotes, SessionInfo,
// 	ValidationCode, ValidationCodeHash, ValidatorId, ValidatorIndex, ValidatorSignature,
// };

use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	Perbill, Permill, Percent, curve::PiecewiseLinear,
	traits::{AccountIdLookup, BlakeTwo256, Block as BlockT, IdentifyAccount, NumberFor, OpaqueKeys, Verify, ConstU16, Replace, TypedGet},
	transaction_validity::{TransactionSource, TransactionValidity, TransactionPriority},
	ApplyExtrinsicResult, MultiSignature,
};

use sp_std::marker::PhantomData;

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use runtime_common::{
	auctions,
	// claims,
	// crowdloan,
	paras_registrar,
	// prod_or_fast, slots, BlockHashCount, BlockLength, CurrencyToVote, SlowAdjustingFeeUpdate,
	prod_or_fast,
	slots,
};
use runtime_parachains::{
	configuration as parachains_configuration, disputes as parachains_disputes,
	dmp as parachains_dmp, hrmp as parachains_hrmp, inclusion as parachains_inclusion,
	initializer as parachains_initializer, origin as parachains_origin, paras as parachains_paras,
	paras_inherent as parachains_paras_inherent, reward_points as parachains_reward_points,
	runtime_api_impl::v2 as parachains_runtime_api_impl, scheduler as parachains_scheduler,
	session_info as parachains_session_info, shared as parachains_shared, ump as parachains_ump,
};
// pub mod xcm_config;


mod weights;

// A few exports that help ease life for downstream crates.
pub use frame_support::{
	construct_runtime, parameter_types, assert_ok, ord_parameter_types, PalletId, StorageValue,
	traits::{ConstU128, ConstU8, KeyOwnerProofSystem, Randomness, StorageInfo, LockIdentifier,
			 ConstU32, ConstU64, Contains, EqualPrivilegeOnly, OnInitialize, OriginTrait, Polling,
			 PreimageRecipient, SortedMembers, VoteTally, EnsureOneOf, Everything, InstanceFilter,
			 MapSuccess, TryMapSuccess, Get
	},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		IdentityFee, Weight,
	},
};

use frame_election_provider_support::{onchain, SequentialPhragmen, VoteWeight};

pub use frame_system::{Call as SystemCall, EnsureRoot, EnsureSigned};
pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;

use pallet_transaction_payment::CurrencyAdapter;
use pallet_grandpa::{
	fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList,
};

pub mod governance;
use governance::{
	// old::CouncilCollective,
	pallet_custom_origins,
	// AuctionAdmin, GeneralAdmin,
	LeaseAdmin,
	// StakingAdmin, TreasurySpender,
};

pub const fn deposit(items: u32, bytes: u32) -> Balance {
	items as Balance * 2_000 * CENTS + (bytes as Balance) * 100 * MILLICENTS
}

// #[macro_export]
// macro_rules! prod_or_fast {
// 	($prod:expr, $test:expr) => {
// 		if cfg!(feature = "fast-runtime") {
// 			$test
// 		} else {
// 			$prod
// 		}
// 	};
// 	($prod:expr, $test:expr, $env:expr) => {
// 		if cfg!(feature = "fast-runtime") {
// 			core::option_env!($env).map(|s| s.parse().ok()).flatten().unwrap_or($test)
// 		} else {
// 			$prod
// 		}
// 	};
// }

/// An index to a block.
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
	spec_version: 100,
	impl_version: 1,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 1,
};

/// This determines the average expected block time that we are targeting.
/// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
/// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
/// up by `pallet_aura` to implement `fn slot_duration()`.
///
/// Change this to adjust the block time.
pub const MILLISECS_PER_BLOCK: u64 = 6000;

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
pub const DOLLARS: Balance = UNIT; // 1_000_000_000_000
pub const CENTS: Balance = DOLLARS / 100; // 10_000_000_000
pub const QUID: Balance = CENTS * 100;
pub const GRAND: Balance = QUID * 1_000;
pub const MILLICENTS: Balance = CENTS / 1_000; // 10_000_000

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;
	pub const BlockHashCount: BlockNumber = 2400;
	/// We allow for 2 seconds of compute with a 6 second average block time.
	pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights
		::with_sensible_defaults(2 * WEIGHT_PER_SECOND, NORMAL_DISPATCH_RATIO);
	pub BlockLength: frame_system::limits::BlockLength = frame_system::limits::BlockLength
		::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub const SS58Prefix: u8 = 42;
}

// Configure FRAME pallets to include in runtime.

impl frame_system::Config for Runtime {
	/// The basic call filter to use in dispatchable.
	type BaseCallFilter = frame_support::traits::Everything;
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = BlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = BlockLength;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The aggregated dispatch type that is available for extrinsics.
	type Call = Call;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = AccountIdLookup<AccountId, ()>;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The header type.
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// The ubiquitous event type.
	type Event = Event;
	/// The ubiquitous origin type.
	type Origin = Origin;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Version of the runtime.
	type Version = Version;
	/// Converts a module to the index of the module in `construct_runtime!`.
	///
	/// This type is being generated by `construct_runtime!`.
	type PalletInfo = PalletInfo;
	/// What to do if a new account is created.
	type OnNewAccount = ();
	/// What to do if an account is fully reaped from the system.
	type OnKilledAccount = ();
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// Weight information for the extrinsics of this pallet.
	type SystemWeightInfo = ();
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	/// The set code logic, just the default since we're not a parachain.
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_randomness_collective_flip::Config for Runtime {}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = ConstU32<32>;
}

impl pallet_grandpa::Config for Runtime {
	type Event = Event;
	type Call = Call;

	type KeyOwnerProofSystem = ();

	type KeyOwnerProof =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

	type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
		KeyTypeId,
		GrandpaId,
	)>>::IdentificationTuple;

	type HandleEquivocation = ();

	type WeightInfo = ();
	type MaxAuthorities = ConstU32<32>;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Aura;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const MaxScheduledPerBlock: u32 = 50;
}

impl pallet_scheduler::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type PalletsOrigin = OriginCaller;
	type Call = Call;
	type MaximumWeight = ();
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = ();
	type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
	type PreimageProvider = ();
	type NoPreimagePostponement = ();
}

parameter_types! {
	pub const SpendPeriod: BlockNumber = 24 * DAYS;
	pub const Burn: Permill = Permill::from_percent(1);
	pub const DataDepositPerByte: Balance = 1 * CENTS;

	pub const BountyDepositBase: Balance = 1 * DOLLARS;
	pub const BountyDepositPayoutDelay: BlockNumber = 8 * DAYS;
	pub const BountyUpdatePeriod: BlockNumber = 90 * DAYS;
	pub const MaximumReasonLength: u32 = 16384;
	pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
	pub const CuratorDepositMin: Balance = 10 * DOLLARS;
	pub const CuratorDepositMax: Balance = 200 * DOLLARS;
	pub const BountyValueMinimum: Balance = 10 * DOLLARS;
}

impl pallet_bounties::Config for Runtime {
	type Event = Event;
	type BountyDepositBase = BountyDepositBase;
	type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
	type BountyUpdatePeriod = BountyUpdatePeriod;
	type CuratorDepositMultiplier = CuratorDepositMultiplier;
	type CuratorDepositMin = CuratorDepositMin;
	type CuratorDepositMax = CuratorDepositMax;
	type BountyValueMinimum = BountyValueMinimum;
	type ChildBountyManager = ();
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type WeightInfo = ();
}

type ApproveOrigin = EnsureOneOf<
		EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
	>;

type MoreThanHalfCouncil = EnsureOneOf<
		EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
	>;

parameter_types! {
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 100 * DOLLARS;
	pub const ProposalBondMaximum: Balance = 500 * DOLLARS;
	pub const MaxApprovals: u32 = 100;
}
pub struct SpendOrigin;
impl frame_support::traits::EnsureOrigin<Origin> for SpendOrigin {
	type Success = u128;
	fn try_origin(o: Origin) -> Result<Self::Success, Origin> {
		Result::<frame_system::RawOrigin<_>, Origin>::from(o).and_then(|o| match o {
			frame_system::RawOrigin::Root => Ok(u128::max_value()),
			r => Err(Origin::from(r)),
		})
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<Origin, ()> {
		Ok(Origin::root())
	}
}

impl pallet_treasury::Config for Runtime {
	type PalletId = TreasuryPalletId;
	type Currency = Balances;
	type ApproveOrigin = ApproveOrigin;
	type RejectOrigin = MoreThanHalfCouncil;
	type SpendOrigin = SpendOrigin;
	type Event = Event;
	type OnSlash = Treasury;
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type ProposalBondMaximum = ProposalBondMaximum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = ();
	type SpendFunds = Bounties;
	type MaxApprovals = MaxApprovals;
	type WeightInfo = ();
}

pub type CouncilCollective = pallet_collective::Instance1;
pub type TechnicalCollective = pallet_collective::Instance2;
parameter_types! {
	pub CouncilMotionDuration: BlockNumber = prod_or_fast!(7 * DAYS, 2 * MINUTES, "DOT_MOTION_DURATION");
	pub const CouncilMaxProposals: u32 = 100;
	pub const CouncilMaxMembers: u32 = 100;
}

impl pallet_collective::Config<CouncilCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = CouncilMotionDuration;
	type MaxProposals = CouncilMaxProposals;
	type MaxMembers = CouncilMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = ();
}
parameter_types! {
	pub const TechnicalMotionDuration: BlockNumber = 7 * DAYS;
	pub const TechnicalMaxProposals: u32 = 100;
	pub const TechnicalMaxMembers: u32 = 100;
}
impl pallet_collective::Config<TechnicalCollective> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = TechnicalMotionDuration;
	type MaxProposals = TechnicalMaxProposals;
	type MaxMembers = TechnicalMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = ();
}

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
}

parameter_types! {
	pub const PreimageMaxSize: u32 = 4096 * 1024;
	pub const PreimageBaseDeposit: Balance = deposit(2, 64);
	pub const PreimageByteDeposit: Balance = deposit(0, 1);
}

impl pallet_democracy::Config for Runtime {
	type Proposal = Call;
	type Event = Event;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type VoteLockingPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type MinimumDeposit = MinimumDeposit;
	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = EnsureOneOf<
			pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>,
		frame_system::EnsureRoot<AccountId>,
		>;
	/// A 60% super-majority can have the next scheduled referendum be a straight majority-carries vote.
	type ExternalMajorityOrigin = EnsureOneOf<
			pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 5>,
		frame_system::EnsureRoot<AccountId>,
		>;
	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	type ExternalDefaultOrigin = EnsureOneOf<
			pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>,
		frame_system::EnsureRoot<AccountId>,
		>;
	/// Two thirds of the technical committee can have an `ExternalMajority/ExternalDefault` vote
	/// be tabled immediately and with a shorter voting/enactment period.
	type FastTrackOrigin = EnsureOneOf<
			pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>,
		frame_system::EnsureRoot<AccountId>,
		>;
	type InstantOrigin = EnsureOneOf<
			pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
		frame_system::EnsureRoot<AccountId>,
		>;
	type InstantAllowed = InstantAllowed;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = EnsureOneOf<
			pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>,
		EnsureRoot<AccountId>,
		>;
	// To cancel a proposal before it has been passed, the technical committee must be unanimous or
	// Root must agree.
	type CancelProposalOrigin = EnsureOneOf<
			pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 1, 1>,
		EnsureRoot<AccountId>,
		>;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	// Any single technical committee member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cooloff period.
	type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
	type CooloffPeriod = CooloffPeriod;
	type PreimageByteDeposit = PreimageByteDeposit;
	type OperationalPreimageOrigin = pallet_collective::EnsureMember<AccountId, CouncilCollective>;
	type Slash = Treasury;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type MaxVotes = MaxVotes;
	type WeightInfo = ();
	type MaxProposals = MaxProposals;
}

parameter_types! {
	pub const TipCountdown: BlockNumber = 1 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(20);
	pub const TipReportDepositBase: Balance = 1 * DOLLARS;
}
impl pallet_tips::Config for Runtime {
	type Event = Event;
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type Tippers = PhragmenElection;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type WeightInfo = ();
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

impl pallet_elections_phragmen::Config for Runtime {
	type Event = Event;
	type PalletId = PhragmenElectionPalletId;
	type Currency = Balances;
	type ChangeMembers = Council;
	type InitializeMembers = Council;
	type CurrencyToVote = frame_support::traits::U128CurrencyToVote;
	type CandidacyBond = CandidacyBond;
	type VotingBondBase = VotingBondBase;
	type VotingBondFactor = VotingBondFactor;
	type LoserCandidate = Treasury;
	type KickedMember = Treasury;
	type DesiredMembers = DesiredMembers;
	type DesiredRunnersUp = DesiredRunnersUp;
	type TermDuration = TermDuration;
	type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<500>;
	type AccountStore = System;
	type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
}

// impl From<pallet_transaction_payment::Event<Runtime>> for Event {
// 	fn from(event: pallet_transaction_payment::Event<Runtime>) -> Self {

// 	}
// }
impl pallet_transaction_payment::Config for Runtime {
	type Event = Event;
	type OnChargeTransaction = CurrencyAdapter<Balances, ()>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = IdentityFee<Balance>;
	type LengthToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ();
}
// #[derive(Encode, Debug, Decode, TypeInfo, Eq, PartialEq, Clone, Default, MaxEncodedLen)]
// pub struct Tally {
// 	pub ayes: u32,
// 	pub nays: u32,
// }

// parameter_types! {
// 	pub const AlarmInterval: u64 = 1;
// }

// impl VoteTally<u32> for Tally {
// 	fn ayes(&self) -> u32 {
// 		self.ayes
// 	}

// 	fn turnout(&self) -> Perbill {
// 		Perbill::from_percent(self.ayes + self.nays)
// 	}

// 	fn approval(&self) -> Perbill {
// 		Perbill::from_rational(self.ayes, self.ayes + self.nays)
// 	}

// 	#[cfg(feature = "runtime-benchmarks")]
// 	fn unanimity() -> Self {
// 		Self { ayes: 100, nays: 0 }
// 	}

// 	#[cfg(feature = "runtime-benchmarks")]
// 	fn from_requirements(turnout: Perbill, approval: Perbill) -> Self {
// 		let turnout = turnout.mul_ceil(100u32);
// 		let ayes = approval.mul_ceil(turnout);
// 		Self { ayes, nays: turnout - ayes }
// 	}
// }

pub struct TracksInfo;
impl pallet_referenda::TracksInfo<Balance, BlockNumber> for TracksInfo {
	// type Id = u8;
	type Id = u16;
	type Origin = <Origin as frame_support::traits::OriginTrait>::PalletsOrigin;
	fn tracks() -> &'static [(Self::Id, pallet_referenda::TrackInfo<Balance, BlockNumber>)] {
		static DATA: [(u16, pallet_referenda::TrackInfo<Balance, BlockNumber>); 1] = [(
			0u16,
			pallet_referenda::TrackInfo {
				name: "root",
				max_deciding: 1,
				decision_deposit: 10,
				prepare_period: 4,
				decision_period: 4,
				confirm_period: 2,
				min_enactment_period: 4,
				min_approval: pallet_referenda::Curve::SteppedDecreasing {
					begin: Perbill::from_percent(100),
					end: Perbill::from_percent(500),
					step: Perbill::from_percent(10),
					period: Perbill::from_percent(1),
				},
				// min_turnout: pallet_referenda::Curve::SteppedDecreasing {
				// 	begin: Perbill::from_percent(100),
				// 	end: Perbill::from_percent(500),
				// 	step: Perbill::from_percent(10),
				// 	period: Perbill::from_percent(1),
				// },
				min_support: pallet_referenda::Curve::LinearDecreasing {
					length: Perbill::from_percent(100),
					floor: Perbill::from_percent(100),
					ceil: Perbill::from_percent(500),
				},
			},
		)];
		&DATA[..]
	}
	fn track_for(id: &Self::Origin) -> Result<Self::Id, ()> {
		if let Ok(system_origin) = frame_system::RawOrigin::try_from(id.clone()) {
			match system_origin {
				frame_system::RawOrigin::Root => Ok(0),
				_ => Err(()),
			}
		} else {
			Err(())
		}
	}
}

impl pallet_referenda::Config for Runtime {
	type Call = Call;
	type Event = Event;
	type WeightInfo = ();
	type Scheduler = Scheduler;
	type Currency = Balances;
	type CancelOrigin = EnsureRoot<AccountId>;
	type KillOrigin = EnsureRoot<AccountId>;
	type SubmitOrigin = EnsureSigned<AccountId>;
	type Slash = ();
	type Votes = pallet_conviction_voting::VotesOf<Runtime>;
	type Tally = pallet_conviction_voting::TallyOf<Runtime>;
	type SubmissionDeposit = ConstU128<2>;
	type MaxQueued = ConstU32<3>;
	type UndecidingTimeout = ConstU32<20>;
	type AlarmInterval = ConstU32<1>;
	type Tracks = TracksInfo;
}

impl pallet_sudo::Config for Runtime {
	type Event = Event;
	type Call = Call;
}

parameter_types! {
	// Six sessions in an era (6 hours).
	pub const SessionsPerEra: sp_staking::SessionIndex = 6;
	// 28 eras for unbonding (7 days).
	pub const BondingDuration: sp_staking::EraIndex = 28;
	// 27 eras in which slashes can be cancelled (slightly less than 7 days).
	pub const SlashDeferDuration: sp_staking::EraIndex = 27;
	pub const MaxNominatorRewardedPerValidator: u32 = 256;
	pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
	// 24
	// pub const MaxNominations: u32 = <NposCompactSolution24 as NposSolution>::LIMIT as u32;
	pub const MaxNominations: u32 = 24;
	pub const Period: u32 = 6 * HOURS;
	pub const Offset: u32 = 0;
}
impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
	Call: From<C>,
{
	type Extrinsic = UncheckedExtrinsic;
	type OverarchingCall = Call;
}

pub struct OnChainSeqPhragmen;
impl onchain::Config for OnChainSeqPhragmen {
	type System = Runtime;
	type Solver = SequentialPhragmen<AccountId, Perbill>;
	type DataProvider = Staking;
	type WeightInfo = ();
}

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
pub struct StakingBenchmarkingConfig;
impl pallet_staking::BenchmarkingConfig for StakingBenchmarkingConfig {
	type MaxValidators = ConstU32<1000>;
	type MaxNominators = ConstU32<1000>;

}
impl pallet_staking::Config for Runtime {
	type MaxNominations = MaxNominations;
	type Currency = Balances;
	type CurrencyBalance = Balance;
	type UnixTime = Timestamp;
	type CurrencyToVote = frame_support::traits::SaturatingCurrencyToVote;
	type ElectionProvider = onchain::UnboundedExecution<OnChainSeqPhragmen>;
	type GenesisElectionProvider = onchain::UnboundedExecution<OnChainSeqPhragmen>;
	type RewardRemainder = Treasury;
	type Event = Event;
	type Slash = ();
	type Reward = ();
	type SessionsPerEra = SessionsPerEra;
	type BondingDuration = BondingDuration;
	type SlashDeferDuration = SlashDeferDuration;
	// A majority of the council or root can cancel the slash.
	type SlashCancelOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type SessionInterface = Self;
	type EraPayout = EraPayout;
	type NextNewSession = Session;
	type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
	type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
	type VoterList = VoterList;
	type MaxUnlockingChunks = frame_support::traits::ConstU32<32>;
	type BenchmarkingConfig = StakingBenchmarkingConfig;
	type OnStakerSlash = ();
	type WeightInfo = ();
}

mod bag_thresholds;
parameter_types! {
	pub const BagThresholds: &'static [u64] = &bag_thresholds::THRESHOLDS;
}

impl pallet_bags_list::Config for Runtime {
	type Event = Event;
	type ScoreProvider = Staking;
	type BagThresholds = BagThresholds;
	type Score = VoteWeight;
	type WeightInfo = ();
}


pub struct TestShouldEndSession;
impl pallet_session::ShouldEndSession<u32> for TestShouldEndSession {
	fn should_end_session(now: u32) -> bool {
		true
	}
}
pub struct TestValidatorIdOf;
impl TestValidatorIdOf {}

impl sp_runtime::traits::Convert<AccountId, Option<AccountId>> for TestValidatorIdOf {
	fn convert(x: AccountId) -> Option<AccountId> {
		Some(x)
	}
}

impl pallet_session::Config for Runtime {
	type Event = Event;
	type ValidatorId = AccountId;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type ValidatorIdOf = TestValidatorIdOf;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = ();
	type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = opaque::SessionKeys;
	type WeightInfo = ();
}

impl pallet_session::historical::Config for Runtime {
	type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

impl pallet_preimage::Config for Runtime {
	type WeightInfo = ();
	type Event = Event;
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type MaxSize = PreimageMaxSize;
	type BaseDeposit = PreimageBaseDeposit;
	type ByteDeposit = PreimageByteDeposit;
}

impl pallet_whitelist::Config for Runtime {
	type Event = Event;
	type Call = Call;
	type WhitelistOrigin = EnsureRoot<AccountId>;
	type DispatchWhitelistedOrigin = EnsureRoot<AccountId>;
	type PreimageProvider = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const VoteLockingPeriod: BlockNumber = 7 * DAYS;
}

impl pallet_conviction_voting::Config for Runtime {
	type WeightInfo = ();
	type Event = Event;
	type Currency = Balances;
	type VoteLockingPeriod = VoteLockingPeriod;
	type MaxVotes = ConstU32<512>;
	type MaxTurnout = frame_support::traits::TotalIssuanceOf<Balances, AccountId>;
	type Polls = Referenda;
}

impl pallet_custom_origins::Config for Runtime {

}

parameter_types! {
	pub const ParaDeposit: Balance = 40 * UNIT;
}

// use frame_support::{traits::Get, weights::Weight};
// use sp_std::marker::PhantomData;
////////////////////////////////////////////////////////////////////////////////
impl pallet_authority_discovery::Config for Runtime {
	type MaxAuthorities = MaxAuthorities;
}

impl parachains_origin::Config for Runtime {}

impl parachains_configuration::Config for Runtime {
	type WeightInfo = weights::runtime_parachains_configuration::WeightInfo<Runtime>;
	// type WeightInfo = ();
}

impl parachains_shared::Config for Runtime {}

impl parachains_session_info::Config for Runtime {
	type ValidatorSet = Historical;
}

impl parachains_inclusion::Config for Runtime {
	type Event = Event;
	type DisputesHandler = ParasDisputes;
	type RewardValidators = parachains_reward_points::RewardValidatorsWithEraPoints<Runtime>;
}

parameter_types! {
	pub const ParasUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
}

impl parachains_paras::Config for Runtime {
	type Event = Event;
	type WeightInfo = weights::runtime_parachains_paras::WeightInfo<Runtime>;
	// type WeightInfo = ();
	type UnsignedPriority = ParasUnsignedPriority;
	// type NextSessionRotation = Babe;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
}

parameter_types! {
	pub const FirstMessageFactorPercent: u64 = 100;
	pub const MaxAuthorities: u32 = 100_000;
}

impl parachains_ump::Config for Runtime {
	type Event = Event;
	// type UmpSink =
	// 	crate::parachains_ump::XcmSink<xcm_executor::XcmExecutor<xcm_config::XcmConfig>, Runtime>;
	type UmpSink = ();
	type FirstMessageFactorPercent = FirstMessageFactorPercent;
	type ExecuteOverweightOrigin = EnsureRoot<AccountId>;
	type WeightInfo = weights::runtime_parachains_ump::WeightInfo<Runtime>;
	// type WeightInfo = ();
}

impl parachains_dmp::Config for Runtime {}

impl parachains_hrmp::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type Currency = Balances;
	type WeightInfo = weights::runtime_parachains_hrmp::WeightInfo<Self>;
	// type WeightInfo = ();
}

// impl pallet_babe::Config for Runtime {
// 	type EpochDuration = EpochDurationInBlocks;
// 	// type EpochDuration = 10;
// 	type ExpectedBlockTime = ExpectedBlockTime;

// 	// session module is the trigger
// 	type EpochChangeTrigger = pallet_babe::ExternalTrigger;

// 	type DisabledValidators = Session;

// 	type KeyOwnerProofSystem = Historical;

// 	type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
// 		KeyTypeId,
// 		pallet_babe::AuthorityId,
// 	)>>::Proof;

// 	type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
// 		KeyTypeId,
// 		pallet_babe::AuthorityId,
// 	)>>::IdentificationTuple;

// 	type HandleEquivocation =
// 		pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, Offences, ReportLongevity>;

// 	type WeightInfo = ();

// 	type MaxAuthorities = MaxAuthorities;

// }

// impl parachains_paras_inherent::Config for Runtime {
// 	// type WeightInfo = weights::runtime_parachains_paras_inherent::WeightInfo<Runtime>;
// 	type WeightInfo = ();
// }

impl parachains_scheduler::Config for Runtime {}

impl parachains_initializer::Config for Runtime {
	// type Randomness = pallet_babe::RandomnessFromOneEpochAgo<Runtime>;
	//type Randomness = ParachainInitializerRandomness;
	type Randomness = RandomnessCollectiveFlip;
	// type Randomness = ();
	type ForceOrigin = EnsureRoot<AccountId>;
	type WeightInfo = weights::runtime_parachains_initializer::WeightInfo<Runtime>;
	// type WeightInfo = ();
}

impl parachains_disputes::Config for Runtime {
	type Event = Event;
	type RewardValidators = ();
	type PunishValidators = ();
	type WeightInfo = weights::runtime_parachains_disputes::WeightInfo<Runtime>;
	// type WeightInfo = ();
}

/// Weight functions for `runtime_common::paras_registrar`.
pub struct ParasRegistrarWeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> runtime_common::paras_registrar::WeightInfo for ParasRegistrarWeightInfo<T> {
	// Storage: Registrar NextFreeParaId (r:1 w:1)
	// Storage: Registrar Paras (r:1 w:1)
	// Storage: Paras ParaLifecycles (r:1 w:0)
	fn reserve() -> Weight {
		(23_252_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
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
		(8_592_152_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(7 as Weight))
			.saturating_add(T::DbWeight::get().writes(7 as Weight))
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
		(8_532_677_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(7 as Weight))
			.saturating_add(T::DbWeight::get().writes(7 as Weight))
	}
	// Storage: Registrar Paras (r:1 w:1)
	// Storage: Paras ParaLifecycles (r:1 w:1)
	// Storage: Paras FutureCodeHash (r:1 w:0)
	// Storage: ParasShared CurrentSessionIndex (r:1 w:0)
	// Storage: Paras ActionsQueue (r:1 w:1)
	// Storage: Registrar PendingSwap (r:0 w:1)
	fn deregister() -> Weight {
		(45_897_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(5 as Weight))
			.saturating_add(T::DbWeight::get().writes(4 as Weight))
	}
	// Storage: Registrar Paras (r:1 w:0)
	// Storage: Paras ParaLifecycles (r:2 w:2)
	// Storage: Registrar PendingSwap (r:1 w:1)
	// Storage: ParasShared CurrentSessionIndex (r:1 w:0)
	// Storage: Paras ActionsQueue (r:1 w:1)
	// Storage: Crowdloan Funds (r:2 w:2)
	// Storage: Slots Leases (r:2 w:2)
	fn swap() -> Weight {
		(38_065_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(10 as Weight))
			.saturating_add(T::DbWeight::get().writes(8 as Weight))
	}
}

impl paras_registrar::Config for Runtime {
	type Event = Event;
	type Origin = Origin;
	type Currency = Balances;
	type OnSwap = (Slots,);
	type ParaDeposit = ParaDeposit;
	type DataDepositPerByte = DataDepositPerByte;
	type WeightInfo = weights::runtime_common_paras_registrar::WeightInfo<Runtime>;
	// type WeightInfo = ParasRegistrarWeightInfo<Runtime>;
	// type WeightInfo = ();
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
		(23_852_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	// Storage: Paras Parachains (r:1 w:0)
	// Storage: Slots Leases (r:101 w:100)
	// Storage: Paras ParaLifecycles (r:101 w:101)
	// Storage: ParasShared CurrentSessionIndex (r:1 w:0)
	// Storage: Paras ActionsQueue (r:1 w:1)
	// Storage: Registrar Paras (r:100 w:100)
	fn manage_lease_period_start(c: u32, t: u32, ) -> Weight {
		(0 as Weight)
			// Standard Error: 19_000
			.saturating_add((7_061_000 as Weight).saturating_mul(c as Weight))
			// Standard Error: 19_000
			.saturating_add((17_484_000 as Weight).saturating_mul(t as Weight))
			.saturating_add(T::DbWeight::get().reads(4 as Weight))
			.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(c as Weight)))
			.saturating_add(T::DbWeight::get().reads((3 as Weight).saturating_mul(t as Weight)))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
			.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(c as Weight)))
			.saturating_add(T::DbWeight::get().writes((3 as Weight).saturating_mul(t as Weight)))
	}
	// Storage: Slots Leases (r:1 w:1)
	// Storage: System Account (r:8 w:8)
	fn clear_all_leases() -> Weight {
		(93_598_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(9 as Weight))
			.saturating_add(T::DbWeight::get().writes(9 as Weight))
	}
	// Storage: Slots Leases (r:1 w:0)
	// Storage: Paras ParaLifecycles (r:1 w:1)
	// Storage: ParasShared CurrentSessionIndex (r:1 w:0)
	// Storage: Paras ActionsQueue (r:1 w:1)
	// Storage: Registrar Paras (r:1 w:1)
	fn trigger_onboard() -> Weight {
		(22_196_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(5 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
}

impl slots::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type Registrar = Registrar;
	type LeasePeriod = LeasePeriod;
	type LeaseOffset = ();
	type ForceOrigin = LeaseAdmin;
	type WeightInfo = SlotsWeightInfo<Runtime>;

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

		// Token related
		Balances: pallet_balances,
		TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>} = 33,
		Bounties: pallet_bounties,
		Tips: pallet_tips,
		Session: pallet_session,
		Staking: pallet_staking,
		Historical: pallet_session::historical,
		AuthorityDiscovery: pallet_authority_discovery::{Pallet, Config} = 12,

		VoterList: pallet_bags_list::{Pallet, Call, Storage, Event<T>} = 39,

		Preimage: pallet_preimage,
		Whitelist: pallet_whitelist,

		// Governance
		Democracy: pallet_democracy,
		Council: pallet_collective::<Instance1>,
		TechnicalCommittee: pallet_collective::<Instance2>,
		PhragmenElection: pallet_elections_phragmen,
		Treasury: pallet_treasury,
		ConvictionVoting: pallet_conviction_voting::{Pallet, Call, Storage, Event<T>} = 20,
		Referenda: pallet_referenda,
		FellowshipCollective: pallet_ranked_collective::<Instance1>,
		FellowshipReferenda: pallet_referenda::<Instance2>,

		Origins: pallet_custom_origins::{Origin},

		// Parachains pallets. Start indices at 50 to leave room.
		ParachainsOrigin: parachains_origin::{Pallet, Origin} = 50,
		Configuration: parachains_configuration::{Pallet, Call, Storage, Config<T>} = 51,
		ParasShared: parachains_shared::{Pallet, Call, Storage} = 52,
		ParaInclusion: parachains_inclusion::{Pallet, Call, Storage, Event<T>} = 53,
		// ParaInherent: parachains_paras_inherent::{Pallet, Call, Storage, Inherent} = 54,
		ParaScheduler: parachains_scheduler::{Pallet, Storage} = 55,
		Paras: parachains_paras::{Pallet, Call, Storage, Event, Config} = 56,
		Initializer: parachains_initializer::{Pallet, Call, Storage} = 57,
		Dmp: parachains_dmp::{Pallet, Call, Storage} = 58,
		Ump: parachains_ump::{Pallet, Call, Storage, Event} = 59,
		Hrmp: parachains_hrmp::{Pallet, Call, Storage, Event<T>, Config} = 60,
		ParaSessionInfo: parachains_session_info::{Pallet, Storage} = 61,
		ParasDisputes: parachains_disputes::{Pallet, Call, Storage, Event<T>} = 62,

		// Parachain Onboarding Pallets. Start indices at 70 to leave room.
		Registrar: paras_registrar::{Pallet, Call, Storage, Event<T>} = 70,
		Slots: slots::{Pallet, Call, Storage, Event<T>} = 71,
		// Auctions: auctions::{Pallet, Call, Storage, Event<T>} = 72,
		// Crowdloan: crowdloan::{Pallet, Call, Storage, Event<T>} = 73,

		Sudo: pallet_sudo,
		// Pallet for sending XCM,
		// XcmPallet: pallet_xcm = 99,
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
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
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
				// Event Count
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
