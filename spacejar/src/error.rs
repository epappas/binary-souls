/// Core error types for the ML runtime system
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
	#[error("Model error: {0}")]
	Model(String),
	#[error("Blockchain error: {0}")]
	Blockchain(String),
	#[error("Data error: {0}")]
	Data(String),
	#[error("System error: {0}")]
	System(String),
}
