use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("io error")]
	IoError(#[from] std::io::Error),
	#[error("runtime error")]
	RuntimeError(String, String),
	#[error("wrong path")]
	InvalidPath,
    #[error("unknown data store error")]
    Unknown,
}