use thiserror::Error;

#[derive(Error, Debug)]
pub enum GraphicsError {
    #[error("Device initialization failed: {0}")]
    DeviceInitializationFailed(String),
}
