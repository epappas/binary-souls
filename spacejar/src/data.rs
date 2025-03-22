use async_trait::async_trait;

use crate::error::RuntimeError;

/// Trait for data management operations
#[async_trait]
pub trait DataManager: Send + Sync {
	/// Store data with optional encryption
	async fn store_data(&self, key: &str, data: Vec<u8>, encrypt: bool)
		-> Result<(), RuntimeError>;

	/// Retrieve data and decrypt if necessary
	async fn retrieve_data(&self, key: &str) -> Result<Vec<u8>, RuntimeError>;

	/// Delete data from storage
	async fn delete_data(&self, key: &str) -> Result<(), RuntimeError>;
}
