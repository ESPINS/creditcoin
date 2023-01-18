use super::{Migrate, PhantomData};
use crate::pallet::WeightInfo;
use crate::types::Task;
use crate::Config;
use crate::StorageVersion;
use frame_support::weights::Weight;
use pallet_offchain_task_scheduler::tasks::TaskScheduler;
use pallet_offchain_task_scheduler::tasks::TaskV2;
use sp_runtime::traits::UniqueSaturatedInto;

pub(super) struct Migration<Runtime>(PhantomData<Runtime>);

impl<Runtime> Migration<Runtime> {
	pub(super) fn new() -> Self {
		Self(PhantomData)
	}
}

impl<T: Config> Migrate for Migration<T> {
	fn pre_upgrade(&self) {}

	fn migrate(&self) -> Weight {
		let mut n = 0u32;
		for (i, (k1, _, v)) in crate::PendingTasks::<T>::drain().enumerate() {
			n = i.unique_saturated_into();
			let id: T::Hash = match &v {
				Task::CollectCoins(pending) => TaskV2::<T>::to_id(pending),
				Task::VerifyTransfer(pending) => TaskV2::<T>::to_id(pending),
			};

			T::TaskScheduler::insert(&k1, &id, v);
		}
		crate::weights::WeightInfo::<T>::migration_v7(n)
	}
	fn post_upgrade(&self) {
		assert_eq!(
			StorageVersion::get::<crate::Pallet<T>>(),
			7,
			"expected storage version to be 7 after migrations complete"
		);
	}
}

#[cfg(test)]
pub mod tests {
	use super::Migrate;
	use super::*;
	use crate::helpers::extensions::IntoBounded;
	use crate::mock::ExtBuilder;
	use crate::mock::Test;
	use crate::test::create_unverified_transfer;
	use crate::types;
	use crate::CollectedCoinsId;
	use crate::TransferId;

	#[test]
	fn migrate_collect_coins() {
		ExtBuilder::default().build_and_execute(|| {
			let pending = types::UnverifiedCollectedCoins {
				to: [0u8; 256].into_bounded(),
				tx_id: [0u8; 256].into_bounded(),
				contract: Default::default(),
			};
			let id = TaskV2::<Test>::to_id(&pending);

			crate::PendingTasks::<Test>::insert(
				1u64,
				crate::TaskId::from(CollectedCoinsId::from(id)),
				Task::from(pending.clone()),
			);

			super::Migration::<Test>::new().migrate();

			let migrated_pending = {
				if let Task::CollectCoins(pending) =
					pallet_offchain_task_scheduler::pallet::PendingTasks::<Test>::get(1, id)
						.unwrap()
				{
					pending
				} else {
					unreachable!()
				}
			};
			assert_eq!(pending, migrated_pending);
		});
	}

	#[test]
	fn migrate_verify_transfer() {
		ExtBuilder::default().build_and_execute(|| {
			let pending = create_unverified_transfer();

			let id = TaskV2::<Test>::to_id(&pending);

			crate::PendingTasks::<Test>::insert(
				1u64,
				crate::TaskId::from(TransferId::from(id)),
				Task::from(pending.clone()),
			);

			super::Migration::<Test>::new().migrate();

			let migrated_pending = {
				if let Task::VerifyTransfer(pending) =
					pallet_offchain_task_scheduler::pallet::PendingTasks::<Test>::get(1, id)
						.unwrap()
				{
					pending
				} else {
					unreachable!()
				}
			};
			assert_eq!(pending, migrated_pending);
		});
	}
}