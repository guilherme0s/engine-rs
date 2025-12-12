use std::ffi::CString;

use ash::{khr, vk};

pub struct VulkanGraphicsDevice {
    instance: ash::Instance,
}

impl VulkanGraphicsDevice {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let entry = unsafe { ash::Entry::load()? };
        let instance = Self::create_instance(&entry)?;

        Ok(Self { instance })
    }

    fn create_instance(entry: &ash::Entry) -> Result<ash::Instance, Box<dyn std::error::Error>> {
        let app_name = CString::new("MyEngine").unwrap();
        let engine_name = CString::new("Graphics").unwrap();

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(1, 0, 0, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(1, 0, 0, 0))
            .api_version(vk::API_VERSION_1_3);

        let mut extensions = vec![khr::surface::NAME.as_ptr()];

        #[cfg(debug_assertions)]
        {
            extensions.push(ash::ext::debug_utils::NAME.as_ptr())
        }
        #[cfg(target_os = "linux")]
        {
            extensions.push(khr::wayland_surface::NAME.as_ptr());
        }

        let mut create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions);

        let layers: Vec<*const i8> = vec![CString::new("VK_LAYER_KHRONOS_validation")?.as_ptr()];

        if !layers.is_empty() {
            create_info = create_info.enabled_layer_names(&layers)
        }

        let instance = unsafe { entry.create_instance(&create_info, None)? };

        Ok(instance)
    }
}

impl Drop for VulkanGraphicsDevice {
    fn drop(&mut self) {
        unsafe {
            self.instance.destroy_instance(None);
        }
    }
}
