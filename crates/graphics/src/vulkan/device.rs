use ash::{khr, vk};
use std::ffi::{CStr, CString, c_char};

pub struct VulkanGraphicsDevice {
    instance: ash::Instance,
    debug_utils_loader: ash::ext::debug_utils::Instance,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    _physical_device: vk::PhysicalDevice,
    device: ash::Device,
    _graphics_queue: vk::Queue,
    _graphics_family_index: u32,
}

impl VulkanGraphicsDevice {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let entry = unsafe { ash::Entry::load()? };
        let instance = Self::create_instance(&entry)?;

        let (debug_utils_loader, debug_messenger) = Self::setup_debug_messenger(&entry, &instance)?;

        let physical_device = Self::select_physical_device(&instance)?;
        let (device, graphics_queue, graphics_family_index) =
            Self::create_logical_device(&instance, physical_device)?;

        Ok(Self {
            instance,
            debug_utils_loader,
            debug_messenger,
            _physical_device: physical_device,
            device,
            _graphics_queue: graphics_queue,
            _graphics_family_index: graphics_family_index,
        })
    }

    fn create_instance(entry: &ash::Entry) -> Result<ash::Instance, Box<dyn std::error::Error>> {
        let app_name = CString::new("MyEngine")?;
        let engine_name = CString::new("Graphics")?;

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(1, 0, 0, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(1, 0, 0, 0))
            .api_version(vk::API_VERSION_1_3);

        let mut extensions: Vec<*const i8> = Vec::new();
        extensions.push(khr::surface::NAME.as_ptr());
        extensions.push(ash::ext::debug_utils::NAME.as_ptr());

        #[cfg(target_os = "linux")]
        {
            extensions.push(khr::wayland_surface::NAME.as_ptr());
            extensions.push(khr::xlib_surface::NAME.as_ptr());
        }

        let layer_names = [c"VK_LAYER_KHRONOS_validation"];
        let layers_names_raw: Vec<*const c_char> = layer_names
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layers_names_raw);

        let instance = unsafe { entry.create_instance(&create_info, None)? };

        Ok(instance)
    }

    fn setup_debug_messenger(
        entry: &ash::Entry,
        instance: &ash::Instance,
    ) -> Result<
        (ash::ext::debug_utils::Instance, vk::DebugUtilsMessengerEXT),
        Box<dyn std::error::Error>,
    > {
        let loader = ash::ext::debug_utils::Instance::new(entry, &instance);
        let create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
            .message_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                    | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            )
            .message_type(
                vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                    | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                    | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            )
            .pfn_user_callback(Some(vulkan_debug_callback));

        let messenger = unsafe { loader.create_debug_utils_messenger(&create_info, None)? };

        Ok((loader, messenger))
    }

    fn select_physical_device(
        instance: &ash::Instance,
    ) -> Result<vk::PhysicalDevice, Box<dyn std::error::Error>> {
        let devices = unsafe { instance.enumerate_physical_devices()? };

        if devices.is_empty() {
            return Err("No Vulkan-capable GPU found".into());
        }

        struct DeviceInfo {
            index: usize,
            handle: vk::PhysicalDevice,
            properties: vk::PhysicalDeviceProperties,
        }

        let mut device_infos = Vec::new();
        for (i, &device) in devices.iter().enumerate() {
            let properties = unsafe { instance.get_physical_device_properties(device) };
            device_infos.push(DeviceInfo {
                index: i,
                handle: device,
                properties,
            });
        }

        // Priority: Discrete GPU > Integrated GPU > Virtual GPU > CPU > Other
        device_infos.sort_by(|a, b| {
            let a_type = a.properties.device_type;
            let b_type = b.properties.device_type;

            if a_type == b_type {
                a.index.cmp(&b.index)
            } else if a_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                std::cmp::Ordering::Less
            } else if b_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                std::cmp::Ordering::Greater
            } else {
                a.index.cmp(&b.index)
            }
        });

        for info in &device_infos {
            return Ok(info.handle);
        }

        Err("No suitable GPU found".into())
    }

    fn create_logical_device(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
    ) -> Result<(ash::Device, vk::Queue, u32), Box<dyn std::error::Error>> {
        let queue_family_properties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        let mut graphics_family_index = None;
        for (index, info) in queue_family_properties.iter().enumerate() {
            if info.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                graphics_family_index = Some(index as u32);
                break;
            }
        }

        let graphics_family_index: u32 = graphics_family_index
            .ok_or::<Box<dyn std::error::Error>>("No graphics queue family found".into())?;

        let queue_priority = [1.0_f32];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(graphics_family_index)
            .queue_priorities(&queue_priority)];

        let device_features = vk::PhysicalDeviceFeatures::default();
        // TODO: Enable commonly used features for AAA rendering

        let device_extensions = [khr::swapchain::NAME.as_ptr()];

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_info)
            .enabled_features(&device_features)
            .enabled_extension_names(&device_extensions);

        let device = unsafe { instance.create_device(physical_device, &device_create_info, None)? };
        let graphics_queue = unsafe { device.get_device_queue(graphics_family_index, 0) };

        Ok((device, graphics_queue, graphics_family_index))
    }
}

impl Drop for VulkanGraphicsDevice {
    fn drop(&mut self) {
        unsafe {
            // Wait for device to finish all operations before destroying
            let _ = self.device.device_wait_idle();

            self.device.destroy_device(None);
            self.debug_utils_loader
                .destroy_debug_utils_messenger(self.debug_messenger, None);
            self.instance.destroy_instance(None);
        }
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    let message = unsafe { CStr::from_ptr((*p_callback_data).p_message) };
    println!(
        "[{:?}] [{:?}] {:?}",
        message_severity, message_type, message
    );
    vk::FALSE
}
