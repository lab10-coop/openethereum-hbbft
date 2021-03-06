use client_traits::{Balance, Nonce, StateOrBlock};
use common_types::ids::BlockId;
use common_types::transaction::{Action, SignedTransaction, Transaction};
use engine::signer::from_keypair;
use ethcore::client::{ChainSyncing, Client};
use ethcore::miner::{Miner, MinerService};
use ethcore::test_helpers::generate_dummy_client_with_spec;
use ethcore::test_helpers::TestNotify;
use ethereum_types::{Address, U256};
use hbbft::NetworkInfo;
use parity_crypto::publickey::{KeyPair, Public};
use spec::Spec;
use std::sync::Arc;

pub fn hbbft_spec() -> Spec {
	Spec::load(
		&::std::env::temp_dir(),
		include_bytes!("../../res/honey_badger_bft.json") as &[u8],
	)
	.expect(concat!("Chain spec is invalid."))
}

struct SyncProviderWrapper();
impl ChainSyncing for SyncProviderWrapper {
	fn is_major_syncing(&self) -> bool {
		false
	}
}

pub fn hbbft_client() -> std::sync::Arc<ethcore::client::Client> {
	let client = generate_dummy_client_with_spec(hbbft_spec);
	client.set_sync_provider(Box::new(SyncProviderWrapper()));
	client
}

pub struct HbbftTestClient {
	pub client: Arc<Client>,
	pub notify: Arc<TestNotify>,
	pub miner: Arc<Miner>,
	pub keypair: KeyPair,
	pub nonce: U256,
}

impl HbbftTestClient {
	pub fn transfer_to(&mut self, receiver: &Address, amount: &U256) {
		let transaction = create_transfer(&self.keypair, receiver, amount, &self.nonce);
		self.nonce += U256::from(1);
		self.miner
			.import_own_transaction(self.client.as_ref(), transaction.into(), false)
			.unwrap();
	}

	// Trigger a generic transaction to force block creation.
	pub fn create_some_transaction(&mut self, caller: Option<&KeyPair>) {
		let keypair = caller.unwrap_or(&self.keypair);
		let cur_nonce = self
			.client
			.nonce(
				&keypair.address(),
				BlockId::Number(self.client.chain().best_block_number()),
			)
			.expect("Nonce for the current best block must always succeed");
		let transaction = create_transaction(keypair, &cur_nonce);
		self.miner
			.import_own_transaction(self.client.as_ref(), transaction.into(), false)
			.unwrap();
	}

	pub fn call_as(
		&mut self,
		caller: &KeyPair,
		receiver: &Address,
		abi_call: ethabi::Bytes,
		amount: &U256,
	) {
		let cur_nonce = self
			.client
			.nonce(
				&caller.address(),
				BlockId::Number(self.client.chain().best_block_number()),
			)
			.expect("Nonce for the current best block must always succeed");
		let transaction = create_call(caller, receiver, abi_call, amount, &cur_nonce);
		self.miner
			.import_claimed_local_transaction(self.client.as_ref(), transaction.into(), false)
			.unwrap();
	}

	pub fn balance(&self, address: &Address) -> U256 {
		self.client
			.balance(address, StateOrBlock::Block(BlockId::Latest))
			.expect("Querying address balance should always succeed.")
	}

	pub fn address(&self) -> Address {
		self.keypair.address()
	}
}

pub fn create_hbbft_client(keypair: KeyPair) -> HbbftTestClient {
	let client = hbbft_client();
	let miner = client.miner();
	let engine = client.engine();
	let signer = from_keypair(keypair.clone());
	engine.set_signer(Some(signer));
	engine.register_client(Arc::downgrade(&client) as _);
	let notify = Arc::new(TestNotify::default());
	client.add_notify(notify.clone());

	HbbftTestClient {
		client,
		notify,
		miner,
		keypair,
		nonce: U256::from(0),
	}
}

pub fn hbbft_client_setup(keypair: KeyPair, net_info: NetworkInfo<Public>) -> HbbftTestClient {
	assert_eq!(keypair.public(), net_info.our_id());
	let client = hbbft_client();

	// Get miner reference
	let miner = client.miner();

	let engine = client.engine();
	// Set the signer *before* registering the client with the engine.
	let signer = from_keypair(keypair.clone());

	engine.set_signer(Some(signer));
	engine.register_client(Arc::downgrade(&client) as _);

	// Register notify object for capturing consensus messages
	let notify = Arc::new(TestNotify::default());
	client.add_notify(notify.clone());

	HbbftTestClient {
		client,
		notify,
		miner,
		keypair,
		nonce: U256::from(1048576),
	}
}

pub fn create_transaction(keypair: &KeyPair, nonce: &U256) -> SignedTransaction {
	Transaction {
		action: Action::Call(Address::from_low_u64_be(5798439875)),
		value: U256::zero(),
		data: vec![],
		gas: U256::from(100_000),
		gas_price: "10000000000".into(),
		nonce: *nonce,
	}
	.sign(keypair.secret(), None)
}

pub fn create_transfer(
	keypair: &KeyPair,
	receiver: &Address,
	amount: &U256,
	nonce: &U256,
) -> SignedTransaction {
	Transaction {
		action: Action::Call(receiver.clone()),
		value: amount.clone(),
		data: vec![],
		gas: U256::from(100_000),
		gas_price: "10000000000".into(),
		nonce: *nonce,
	}
	.sign(keypair.secret(), None)
}

pub fn create_call(
	keypair: &KeyPair,
	receiver: &Address,
	abi_call: ethabi::Bytes,
	amount: &U256,
	nonce: &U256,
) -> SignedTransaction {
	Transaction {
		action: Action::Call(receiver.clone()),
		value: amount.clone(),
		data: abi_call,
		gas: U256::from(900_000),
		gas_price: "10000000000".into(),
		nonce: *nonce,
	}
	.sign(keypair.secret(), None)
}
