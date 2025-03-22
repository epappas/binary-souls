use async_trait::async_trait;
use chrono::serde::ts_seconds;
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::{error, info, instrument};

use crate::blockchain::BlockchainManager;
use crate::data::DataManager;
use crate::error::RuntimeError;
use crate::model::{ModelId, ModelManager, ModelState};

/// Trait for system observability
#[async_trait]
pub trait Observer: Send + Sync {
	/// Record a metric
	async fn record_metric(&self, name: &str, value: f64) -> Result<(), RuntimeError>;

	/// Log an event
	async fn log_event(&self, event: Event) -> Result<(), RuntimeError>;

	/// Get system health status
	async fn health_check(&self) -> Result<HealthStatus, RuntimeError>;
}

/// System event types for logging and monitoring
#[derive(Debug, Clone, Serialize)]
pub struct Event {
	#[serde(with = "ts_seconds")]
	pub timestamp: chrono::DateTime<chrono::Utc>,
	pub event_type: EventType,
	pub details: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum EventType {
	ModelOperation,
	BlockchainOperation,
	DataOperation,
	SystemStatus,
}

/// System health status
#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
	pub healthy: bool,
	pub message: String,
	#[serde(with = "ts_seconds")]
	pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Runtime configuration for the entire system
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
	/// Maximum number of events to keep in history
	pub max_event_history: usize,
	/// System-wide operation timeout
	pub operation_timeout: Duration,
	/// Number of worker threads for background tasks
	pub worker_threads: usize,
}

/// Runtime state tracking
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum RuntimeState {
	Starting,
	Running,
	Stopping,
	Stopped,
}

/// The core runtime struct that orchestrates all system components
pub struct Runtime {
	/// Current runtime state
	state: Arc<RwLock<RuntimeState>>,
	/// Model management component
	// model_manager: Arc<dyn ModelManager>,
	// /// Blockchain integration component
	// blockchain_manager: Arc<dyn BlockchainManager>,
	// /// Data management component
	// data_manager: Arc<dyn DataManager>,
	// /// System observer for metrics and logging
	// observer: Arc<dyn Observer>,
	/// Runtime configuration
	config: RuntimeConfig,
	/// Event broadcast channel
	event_tx: broadcast::Sender<Event>,
	/// Background task handles
	task_handles: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl Runtime {
	/// Create a new runtime instance with the provided components and configuration
	pub fn new(config: RuntimeConfig) -> Self {
		let (event_tx, _) = broadcast::channel(1000);

		Self {
			state: Arc::new(RwLock::new(RuntimeState::Stopped)),
			config,
			event_tx,
			task_handles: Arc::new(Mutex::new(Vec::new())),
		}
	}

	/// Start the runtime system
	#[instrument(skip(self))]
	pub async fn start(&self) -> Result<(), RuntimeError> {
		let mut state = self.state.write().await;
		if *state != RuntimeState::Stopped {
			return Err(RuntimeError::System("Runtime already running".into()));
		}

		*state = RuntimeState::Starting;
		info!("Starting ML runtime system");

		// Initialize background tasks
		self.spawn_background_tasks().await?;

		*state = RuntimeState::Running;
		info!("ML runtime system started successfully");

		Ok(())
	}

	/// Stop the runtime system
	#[instrument(skip(self))]
	pub async fn stop(&self) -> Result<(), RuntimeError> {
		let mut state = self.state.write().await;
		if *state != RuntimeState::Running {
			return Err(RuntimeError::System("Runtime not running".into()));
		}

		*state = RuntimeState::Stopping;
		info!("Stopping ML runtime system");

		// Stop background tasks
		self.stop_background_tasks().await?;

		*state = RuntimeState::Stopped;
		info!("ML runtime system stopped successfully");

		Ok(())
	}

	/// Spawn background maintenance tasks
	async fn spawn_background_tasks(&self) -> Result<(), RuntimeError> {
		let mut handles = self.task_handles.lock().await;

		// Health check task
		let observer = Arc::clone(&self.observer);
		let timeout = self.config.operation_timeout;
		let health_handle = tokio::spawn(async move {
			loop {
				if let Err(e) = observer.health_check().await {
					error!("Health check failed: {}", e);
				}
				tokio::time::sleep(timeout).await;
			}
		});
		handles.push(health_handle);

		let maintenance_handle = tokio::spawn(async move {
			loop {
				// Perform model maintenance
				tokio::time::sleep(Duration::from_secs(300)).await;
			}
		});
		handles.push(maintenance_handle);

		Ok(())
	}

	/// Stop all background tasks
	async fn stop_background_tasks(&self) -> Result<(), RuntimeError> {
		let mut handles = self.task_handles.lock().await;
		for handle in handles.iter_mut() {
			handle.abort();
		}
		handles.clear();
		Ok(())
	}

	/// Register a new model in the system
	#[instrument(skip(self))]
	pub async fn register_model(&self, id: ModelId, path: String) -> Result<(), RuntimeError> {
		if *self.state.read().await != RuntimeState::Running {
			return Err(RuntimeError::System("Runtime not running".into()));
		}

		// Log the operation
		self.observer
			.log_event(Event {
				timestamp: chrono::Utc::now(),
				event_type: EventType::ModelOperation,
				details: format!("Registering model {}", id.0),
			})
			.await?;

		// Register the model
		self.model_manager.register_model(id.clone(), path).await?;

		Ok(())
	}

	/// Submit a blockchain transaction
	#[instrument(skip(self, tx_data))]
	pub async fn submit_transaction(&self, tx_data: Vec<u8>) -> Result<String, RuntimeError> {
		if *self.state.read().await != RuntimeState::Running {
			return Err(RuntimeError::System("Runtime not running".into()));
		}

		// Submit the transaction
		let tx_id = self.blockchain_manager.submit_transaction(tx_data).await?;

		// Log the operation
		self.observer
			.log_event(Event {
				timestamp: chrono::Utc::now(),
				event_type: EventType::BlockchainOperation,
				details: format!("Submitted transaction {}", tx_id),
			})
			.await?;

		Ok(tx_id)
	}

	/// Store data with optional encryption
	#[instrument(skip(self, data))]
	pub async fn store_data(
		&self,
		key: &str,
		data: Vec<u8>,
		encrypt: bool,
	) -> Result<(), RuntimeError> {
		if *self.state.read().await != RuntimeState::Running {
			return Err(RuntimeError::System("Runtime not running".into()));
		}

		// Store the data
		self.data_manager.store_data(key, data, encrypt).await?;

		// Log the operation
		self.observer
			.log_event(Event {
				timestamp: chrono::Utc::now(),
				event_type: EventType::DataOperation,
				details: format!("Stored data with key {}", key),
			})
			.await?;

		Ok(())
	}

	/// Subscribe to system events
	pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
		self.event_tx.subscribe()
	}

	/// Get current runtime metrics
	pub async fn get_metrics(&self) -> Result<RuntimeMetrics, RuntimeError> {
		Ok(RuntimeMetrics {
			state: self.state.read().await.clone(),
			active_models: self.count_active_models().await?,
			memory_usage: self.calculate_memory_usage().await?,
			uptime: self.calculate_uptime().await,
		})
	}

	/// Count the number of active models in the system
	async fn count_active_models(&self) -> Result<usize, RuntimeError> {
		let models = self.model_manager.list_models().await?;
		let active_count =
			models.iter().filter(|(_, state)| matches!(state, ModelState::Ready)).count();
		Ok(active_count)
	}

	/// Calculate approximate memory usage of the runtime
	async fn calculate_memory_usage(&self) -> Result<usize, RuntimeError> {
		// This is a simplified implementation that should be replaced
		// with actual memory profiling in a production environment
		let mut total_memory = 0;

		// Get memory usage from model manager
		let models = self.model_manager.list_models().await?;
		for (model_id, _) in models {
			if let Ok(stats) = self.model_manager.get_model_stats(&model_id).await {
				total_memory += stats.memory_usage;
			}
		}

		// Add estimated memory for runtime components
		total_memory += std::mem::size_of::<Runtime>();

		Ok(total_memory)
	}

	/// Calculate the runtime's uptime since start
	async fn calculate_uptime(&self) -> Duration {
		// Note: In a real implementation, you would want to store the start time
		// when transitioning to Running state and calculate based on that
		static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

		let start_time = START_TIME.get_or_init(std::time::Instant::now);
		start_time.elapsed()
	}
}

/// Runtime metrics for monitoring
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeMetrics {
	pub state: RuntimeState,
	pub active_models: usize,
	pub memory_usage: usize,
	pub uptime: Duration,
}
