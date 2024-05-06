use serde::{Deserialize, Serialize};
use crate::{transaction::Transaction};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Mempool {
    transactions: Vec<Transaction>
}
