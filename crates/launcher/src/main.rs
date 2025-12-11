use graphics::{error::GraphicsError, vulkan::device::VulkanGraphicsDevice};

fn main() -> Result<(), GraphicsError> {
    let _device = VulkanGraphicsDevice::new()?;
    Ok(())
}
