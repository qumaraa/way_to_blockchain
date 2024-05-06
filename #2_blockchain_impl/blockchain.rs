use chrono::Utc;
use log::{error, warn};
use crate::block::{Block, calculate_hash, hash_to_binary_representation};
use crate::DIFFICULTY_PREFIX;

pub struct Blockchain {
    pub mining_reward: f32,
    pub blocks: Vec<Block>,
}



impl Blockchain {
    pub fn new() -> Self {
        Self { mining_reward: 10.0, blocks: vec![] }
    }

    pub(crate) fn genesis(&mut self) {
        let genesis_block = Block {
            id: 0,
            timestamp: Utc::now().timestamp(),
            previous_hash: String::from("0000000000000000000000000000000000000000000000000000000000000000"),
            nonce: 0,
            hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            data: "Genesis".to_string(),
            transactions: vec![],
        };
        self.blocks.push(genesis_block);
    }

    pub fn try_add_block(&mut self,block: Block) {
        let latest_block = self.blocks.last().expect("there is at least one block.");
        if self.is_block_valid(&block, latest_block) {
            self.blocks.push(block);
        }else {
            error!("could not add block - invalid");
        }
    }

    pub fn is_block_valid(&self, block: &Block, previous_block: &Block) -> bool {
        if block.previous_hash != previous_block.hash {
            warn!("block with id#{} has wrong previous hash",block.id);
            return false;
        }else if !hash_to_binary_representation(
            &hex::decode(&block.hash).expect("can decode from hex"),
        ).starts_with(DIFFICULTY_PREFIX){
            warn!(
                "block with id#{} is not the next block after the latest: {}",
                block.id, previous_block.id
            );
            return false;
        }else if hex::encode(calculate_hash(
            block.id,
            block.timestamp,
            &block.previous_hash,
            &block.data,
            block.nonce,
        )) != block.hash
        {
            warn!("block with id#{} has invalid hash",block.id);
            return false;
        }
        true
    }
    fn is_chain_valid(&self, chain: &[Block]) -> bool {
        for i in 0..chain.len() {
            if i == 0 {
                continue; // skip the genesis block
            }
            let first = chain.get(i - 1).expect("has to exist");
            let second = chain.get(i).expect("has to exist");
            if !self.is_block_valid(second, first) {
                return false;
            }
        }
        true
    }
    pub(crate) fn choose_chain(&mut self, local: Vec<Block>, remote: Vec<Block>) -> Vec<Block> {
        let is_local_valid = self.is_chain_valid(&local);
        let is_remote_valid = self.is_chain_valid(&remote);

        if is_local_valid && is_remote_valid {
            if local.len() >= remote.len() {
                local
            }else {
                remote
            }
        }else if is_remote_valid && !is_local_valid {
            remote
        }else if !is_remote_valid && is_local_valid {
            local
        }else {
            panic!("local and remote chains are both invalid");
        }
    }
}
