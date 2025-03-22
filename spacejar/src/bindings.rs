use pyo3::exceptions::{PyException, PyRuntimeError, PyValueError};
use pyo3::types::{PyBytes, PyDict};
use pyo3::{create_exception, prelude::*};
use std::sync::Arc;
use tokio::runtime::Runtime;

use crate::model::ModelManager;
use crate::runtime::{Runtime as MLRuntime, RuntimeConfig};

/// Python module configuration
#[pymodule]
#[pyo3(name = "model_runtime")]
fn model_runtime(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
	m.add_class::<PyMLRuntime>()?;
	m.add_class::<PyModelConfig>()?;
	Ok(())
}

/// Configuration class for the ML Runtime
#[pyclass]
#[derive(Clone)]
struct PyModelConfig {
	#[pyo3(get, set)]
	max_memory: usize,
	#[pyo3(get, set)]
	max_concurrent_requests: usize,
	#[pyo3(get, set)]
	inference_timeout_ms: u64,
}

#[pymethods]
impl PyModelConfig {
	#[new]
	fn new(
		max_memory: Option<usize>,
		max_concurrent_requests: Option<usize>,
		inference_timeout_ms: Option<u64>,
	) -> Self {
		Self {
			max_memory: max_memory.unwrap_or(1024 * 1024 * 1024), // 1GB default
			max_concurrent_requests: max_concurrent_requests.unwrap_or(10),
			inference_timeout_ms: inference_timeout_ms.unwrap_or(1000),
		}
	}

	fn __repr__(&self) -> PyResult<String> {
		Ok(format!(
			"ModelConfig(max_memory={}, max_concurrent_requests={}, inference_timeout_ms={})",
			self.max_memory, self.max_concurrent_requests, self.inference_timeout_ms
		))
	}
}

/// Main ML Runtime Python wrapper
#[pyclass]
struct PyMLRuntime {
	runtime: Arc<MLRuntime>,
	tokio_runtime: Arc<Runtime>,
}

#[pymethods]
impl PyMLRuntime {
	/// Create a new ML Runtime instance
	#[new]
	fn new(config: PyModelConfig) -> PyResult<Self> {
		// Create tokio runtime for async operations
		let tokio_runtime = Runtime::new().map_err(|e| {
			PyRuntimeError::new_err(format!("Failed to create tokio runtime: {}", e))
		})?;

		// Create the runtime config
		let runtime_config = RuntimeConfig {
			max_event_history: 1000,
			operation_timeout: std::time::Duration::from_secs(30),
			worker_threads: 4,
		};

		// Create the runtime
		let runtime = Arc::new(MLRuntime::new(
			runtime_config,
		));

		Ok(Self { runtime, tokio_runtime: Arc::new(tokio_runtime) })
	}

	/// Start the runtime
	fn start(&self, py: Python<'_>) -> PyResult<()> {
		let runtime = Arc::clone(&self.runtime);
		let tokio_runtime = Arc::clone(&self.tokio_runtime);

		py.allow_threads(move || {
			tokio_runtime.block_on(async move {
				runtime
					.start()
					.await
					.map_err(|e| PyRuntimeError::new_err(format!("Failed to start runtime: {}", e)))
			})
		})
	}

	/// Stop the runtime
	fn stop(&self, py: Python<'_>) -> PyResult<()> {
		let runtime = Arc::clone(&self.runtime);
		let tokio_runtime = Arc::clone(&self.tokio_runtime);

		py.allow_threads(move || {
			tokio_runtime.block_on(async move {
				runtime
					.stop()
					.await
					.map_err(|e| PyRuntimeError::new_err(format!("Failed to stop runtime: {}", e)))
			})
		})
	}

	/// Register a new model
	fn register_model(&self, py: Python<'_>, model_id: String, path: String) -> PyResult<()> {
		let runtime = Arc::clone(&self.runtime);
		let tokio_runtime = Arc::clone(&self.tokio_runtime);

		py.allow_threads(move || {
			tokio_runtime.block_on(async move {
				runtime.register_model(ModelId(model_id), path).await.map_err(|e| {
					PyRuntimeError::new_err(format!("Failed to register model: {}", e))
				})
			})
		})
	}

	/// Submit a blockchain transaction
	fn submit_transaction(&self, py: Python<'_>, data: &PyBytes) -> PyResult<String> {
		let runtime = Arc::clone(&self.runtime);
		let tokio_runtime = Arc::clone(&self.tokio_runtime);
		let tx_data = data.as_bytes().to_vec();

		py.allow_threads(move || {
			tokio_runtime.block_on(async move {
				runtime.submit_transaction(tx_data).await.map_err(|e| {
					PyRuntimeError::new_err(format!("Failed to submit transaction: {}", e))
				})
			})
		})
	}

	/// Store data with optional encryption
	fn store_data(
		&self,
		py: Python<'_>,
		key: String,
		data: &PyBytes,
		encrypt: bool,
	) -> PyResult<()> {
		let runtime = Arc::clone(&self.runtime);
		let tokio_runtime = Arc::clone(&self.tokio_runtime);
		let data = data.as_bytes().to_vec();

		py.allow_threads(move || {
			tokio_runtime.block_on(async move {
				runtime
					.store_data(&key, data, encrypt)
					.await
					.map_err(|e| PyRuntimeError::new_err(format!("Failed to store data: {}", e)))
			})
		})
	}

	/// Get runtime metrics as a dictionary
	fn get_metrics(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
		let runtime = Arc::clone(&self.runtime);
		let tokio_runtime = Arc::clone(&self.tokio_runtime);

		py.allow_threads(move || {
			tokio_runtime.block_on(async move {
				let metrics = runtime.get_metrics().await.map_err(|e| {
					PyRuntimeError::new_err(format!("Failed to get metrics: {}", e))
				})?;

				Python::with_gil(|py| {
					let dict = PyDict::new(py);
					dict.set_item("state", metrics.state.to_string())?;
					dict.set_item("active_models", metrics.active_models)?;
					dict.set_item("memory_usage", metrics.memory_usage)?;
					dict.set_item("uptime_seconds", metrics.uptime.as_secs())?;
					Ok(dict.into())
				})
			})
		})
	}

	fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
		slf.start(slf.py())?;
		Ok(slf)
	}

	fn __exit__(
		&self,
		py: Python<'_>,
		_exc_type: Option<PyObject>,
		_exc_value: Option<PyObject>,
		_traceback: Option<PyObject>,
	) -> PyResult<()> {
		self.stop(py)
	}
}

create_exception!(ml_runtime, MLRuntimeError, PyException);
