use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, sync::Arc};
use tokio::sync::RwLock;

use crate::error::RuntimeError;

/// Represents a unique identifier for ML models in the system
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModelId(pub String);

impl fmt::Display for ModelId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// Represents the current state of a model in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelState {
	/// Model is registered but not loaded
	Registered,
	/// Model is currently loading into memory
	Loading,
	/// Model is loaded and ready for inference
	Ready,
	/// Model failed to load
	Failed { error: String },
}

/// Statistics for a model instance
#[derive(Debug, Clone, Serialize)]
pub struct ModelStats {
	/// Approximate memory usage in bytes
	pub memory_usage: usize,
	/// Number of inferences performed
	pub inference_count: u64,
	/// Average inference time in milliseconds
	pub avg_inference_time: f64,
}

/// Represents a machine learning model in the system
///
///
#[async_trait]
pub trait Model: Send + Sync {
	/// Get the unique identifier for this model
	fn id(&self) -> ModelId;

	/// Get the current state of this model
	fn state(&self) -> ModelState;

	/// Load the model into memory
	async fn load(&self) -> Result<(), String>;

	/// Perform inference on the model
	async fn infer(&self, input: Vec<f32>) -> Result<Vec<f32>, String>;
}

/// Core trait for ML model management
#[async_trait]
pub trait ModelManager: Send + Sync {
	/// Register a new model in the system
	async fn register_model(&self, id: ModelId, path: String) -> Result<(), RuntimeError>;

	/// Load a model into memory
	async fn load_model(&self, id: ModelId) -> Result<(), RuntimeError>;

	/// Unload a model from memory
	async fn unload_model(&self, id: ModelId) -> Result<(), RuntimeError>;

	/// Get the current state of a model
	async fn get_model_state(&self, id: &ModelId) -> Result<ModelState, RuntimeError>;

	/// List all registered models and their states
	async fn list_models(&self) -> Result<HashMap<ModelId, ModelState>, RuntimeError>;

	/// Get statistics for a specific model
	async fn get_model_stats(&self, id: &ModelId) -> Result<ModelStats, RuntimeError>;
}

/// Implementation of a thread-safe model registry
pub struct ModelRegistry {
	models: Arc<RwLock<HashMap<ModelId, ModelState>>>,
}

impl Default for ModelRegistry {
	fn default() -> Self {
		Self::new()
	}
}

impl ModelRegistry {
	/// Create a new model registry with the specified resource configuration
	pub fn new() -> Self {
		Self { models: Arc::new(RwLock::new(HashMap::new())) }
	}

	/// Add a new model to the registry
	pub async fn add_model(&self, id: ModelId) -> Result<(), RuntimeError> {
		let mut models = self.models.write().await;

		models.insert(id, ModelState::Registered);
		Ok(())
	}

	/// List all models in the registry
	pub async fn list_models(&self) -> Result<HashMap<ModelId, ModelState>, RuntimeError> {
		let models = self.models.read().await;
		Ok(models.clone())
	}

	/// Get statistics for a specific model
	pub async fn get_model_stats(&self, id: &ModelId) -> Result<ModelStats, RuntimeError> {
		let models = self.models.read().await;
		if models.contains_key(id) {
			// For now, return basic stats. In a real implementation,
			// you would track actual metrics per model
			Ok(ModelStats {
				memory_usage: std::mem::size_of::<ModelState>(),
				inference_count: 0,
				avg_inference_time: 0.0,
			})
		} else {
			Err(RuntimeError::Model(format!("Model {} not found", id)))
		}
	}
}
