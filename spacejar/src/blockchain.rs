use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::RuntimeError;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum TransactionState {
	Pending,
	Submitted,
	Confirmed(u64), // Block number where transaction was confirmed
	Failed(String), // Error message
	Unknown,
}

impl fmt::Display for TransactionState {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			TransactionState::Pending => write!(f, "Pending"),
			TransactionState::Submitted => write!(f, "Submitted"),
			TransactionState::Confirmed(block) => write!(f, "Confirmed at block {}", block),
			TransactionState::Failed(err) => write!(f, "Failed: {}", err),
			TransactionState::Unknown => write!(f, "Unknown"),
		}
	}
}

/// Trait for blockchain operations
#[async_trait]
pub trait BlockchainManager: Send + Sync {
	/// Submit a transaction to the blockchain
	async fn submit_transaction(&self, tx_data: Vec<u8>) -> Result<String, RuntimeError>;

	/// Get the current state of a transaction
	async fn get_transaction_state(&self, tx_id: &str) -> Result<TransactionState, RuntimeError>;

	/// Verify a transaction proof
	async fn verify_proof(&self, proof: &[u8]) -> Result<bool, RuntimeError>;
}
