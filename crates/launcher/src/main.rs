use graphics::vulkan::device::VulkanGraphicsDevice;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = VulkanGraphicsDevice::new().map_err(|e| println!("{e}"));

    Ok(())
}
