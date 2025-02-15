#![cfg(test)]
use polkadot_sdk::*;

use codec::Encode;
use pallet_ismp::child_trie::{request_receipt_storage_key, response_receipt_storage_key};
use sp_core::{storage::ChildInfo, H256};
use sp_runtime::{traits::BlakeTwo256, StateVersion};
use sp_state_machine::{
	prove_child_read, read_proof_check, Backend, TrieBackend, TrieBackendBuilder,
};
use sp_trie::{
	trie_types::TrieDBMutBuilderV0, KeySpacedDBMut, LayoutV0, PrefixedMemoryDB, StorageProof,
};
use trie_db::{Recorder, Trie, TrieDBBuilder, TrieMut};

const CHILD_KEY_1: &[u8] = b"sub1";

pub(crate) fn test_db() -> (PrefixedMemoryDB<BlakeTwo256>, H256) {
	let child_info = ChildInfo::new_default(CHILD_KEY_1);
	let mut root = H256::default();
	let mut mdb = PrefixedMemoryDB::<BlakeTwo256>::default();
	{
		let mut mdb = KeySpacedDBMut::new(&mut mdb, child_info.keyspace());
		let mut trie = TrieDBMutBuilderV0::new(&mut mdb, &mut root).build();
		trie.insert(b"value3", &[142; 33]).expect("insert failed");
		trie.insert(b"value4", &[124; 33]).expect("insert failed");
	};

	{
		let mut sub_root = Vec::new();
		root.encode_to(&mut sub_root);

		fn build<L: sp_trie::TrieLayout>(
			mut trie: sp_trie::TrieDBMut<L>,
			child_info: &ChildInfo,
			sub_root: &[u8],
		) {
			trie.insert(child_info.prefixed_storage_key().as_slice(), sub_root)
				.expect("insert failed");
			trie.insert(b"key", b"value").expect("insert failed");
			trie.insert(b"value1", &[42]).expect("insert failed");
			trie.insert(b"value2", &[24]).expect("insert failed");
			trie.insert(b":code", b"return 42").expect("insert failed");
			for i in 128u8..255u8 {
				trie.insert(&[i], &[i]).unwrap();
			}
		}

		let trie = TrieDBMutBuilderV0::new(&mut mdb, &mut root).build();
		build(trie, &child_info, &sub_root[..]);
	}
	(mdb, root)
}

pub(crate) fn test_trie() -> TrieBackend<PrefixedMemoryDB<BlakeTwo256>, BlakeTwo256> {
	let (mdb, root) = test_db();

	TrieBackendBuilder::new(mdb, root).build()
}

// Tests that the proof generated by prove_child_read works when verified directly against child
// trie root
#[test]
fn prove_child_read_proof_works_with_child_trie_root() {
	let child_info = ChildInfo::new_default(CHILD_KEY_1);
	let child_info = &child_info;

	// on child trie
	let remote_backend = test_trie();
	let child_root = remote_backend
		.child_storage_root(child_info, std::iter::empty(), StateVersion::V0)
		.0;
	let storage_proof = prove_child_read(remote_backend, child_info, &[b"value3"]).unwrap();
	dbg!(storage_proof.clone().into_nodes().len());

	let db = storage_proof.into_memory_db::<BlakeTwo256>();

	let mut recorder = Recorder::<LayoutV0<BlakeTwo256>>::default();
	let trie = TrieDBBuilder::<LayoutV0<BlakeTwo256>>::new(&db, &child_root)
		.with_recorder(&mut recorder)
		.build();
	let _ = trie.get(b"value3").unwrap();
	let _ = trie.get(b"value2").unwrap();
	let proof_nodes = recorder.drain().into_iter().map(|f| f.data).collect::<Vec<_>>();
	let storage_proof = StorageProof::new(proof_nodes);
	dbg!(storage_proof.clone().into_nodes().len());
	let local_result1 =
		read_proof_check::<BlakeTwo256, _>(child_root, storage_proof.clone(), &[b"value3"])
			.unwrap();
	let local_result2 =
		read_proof_check::<BlakeTwo256, _>(child_root, storage_proof.clone(), &[b"value2"])
			.unwrap();

	assert_eq!(
		local_result1.into_iter().collect::<Vec<_>>(),
		vec![(b"value3".to_vec(), Some(vec![142; 33]))],
	);
	assert_eq!(local_result2.into_iter().collect::<Vec<_>>(), vec![(b"value2".to_vec(), None)]);
}

#[test]
fn child_trie_storage_key_prefix() {
	let request_receipt_storage_prefix = request_receipt_storage_key(H256::zero())[0..32].to_vec();

	let response_receipt_storage_prefix =
		response_receipt_storage_key(H256::zero())[0..32].to_vec();

	dbg!(hex::encode(request_receipt_storage_prefix));

	dbg!(hex::encode(response_receipt_storage_prefix));
}
