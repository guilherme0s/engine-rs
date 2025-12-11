use std::ffi::{CStr, CString};

use ash::{khr, vk};

use crate::error::GraphicsError;

pub struct VulkanGraphicsDevice {
    _entry: ash::Entry,
    instance: ash::Instance,
    debug_utils: Option<ash::ext::debug_utils::Instance>,
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
    _physical_device: vk::PhysicalDevice,
    device: ash::Device,
    _graphics_queue: vk::Queue,
}

impl VulkanGraphicsDevice {
    pub fn new() -> Result<Self, GraphicsError> {
        let entry = unsafe {
            ash::Entry::load().map_err(|e| {
                GraphicsError::DeviceInitializationFailed(format!("Failed to load Vulkan: {:?}", e))
            })?
        };

        let (instance, debug_utils, debug_messenger) = Self::create_instance(&entry)?;

        let physical_device = Self::select_physical_device(&instance)?;
        let (device, graphics_queue) = Self::create_logical_device(&instance, physical_device)?;

        Ok(Self {
            _entry: entry,
            instance,
            debug_utils,
            debug_messenger,
            _physical_device: physical_device,
            device,
            _graphics_queue: graphics_queue,
        })
    }

    fn create_instance(
        entry: &ash::Entry,
    ) -> Result<
        (
            ash::Instance,
            Option<ash::ext::debug_utils::Instance>,
            Option<vk::DebugUtilsMessengerEXT>,
        ),
        GraphicsError,
    > {
        let app_name = CString::new("MyEngine").unwrap();
        let engine_name = CString::new("GAL").unwrap();

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::API_VERSION_1_2);

        let mut extension_names = vec![khr::surface::NAME.as_ptr()];

        #[cfg(target_os = "linux")]
        extension_names.push(khr::wayland_surface::NAME.as_ptr());

        #[cfg(debug_assertions)]
        extension_names.push(ash::ext::debug_utils::NAME.as_ptr());

        let layer_names = vec![CString::new("VK_LAYER_KHRONOS_validation").unwrap()];
        let layer_name_ptrs: Vec<*const i8> =
            layer_names.iter().map(|name| name.as_ptr()).collect();

        let mut create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names);

        if !layer_name_ptrs.is_empty() {
            create_info = create_info.enabled_layer_names(&layer_name_ptrs)
        }

        let instance = unsafe {
            entry.create_instance(&create_info, None).map_err(|e| {
                GraphicsError::DeviceInitializationFailed(format!(
                    "Failed to create instance: {:?}",
                    e
                ))
            })?
        };

        let (debug_utils, debug_messenger) = if cfg!(debug_assertions) {
            let debug_utils = ash::ext::debug_utils::Instance::new(entry, &instance);
            let debug_create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
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

            let debug_messenger = unsafe {
                debug_utils
                    .create_debug_utils_messenger(&debug_create_info, None)
                    .map_err(|e| {
                        GraphicsError::DeviceInitializationFailed(format!(
                            "Failed to create debug messenger: {:?}",
                            e
                        ))
                    })?
            };

            (Some(debug_utils), Some(debug_messenger))
        } else {
            (None, None)
        };

        Ok((instance, debug_utils, debug_messenger))
    }

    fn select_physical_device(
        instance: &ash::Instance,
    ) -> Result<vk::PhysicalDevice, GraphicsError> {
        let devices = unsafe {
            instance.enumerate_physical_devices().map_err(|_| {
                GraphicsError::DeviceInitializationFailed(
                    "Failed to enumerate physical devices".into(),
                )
            })?
        };

        for &device in devices.iter() {
            // TODO: For simplicity, pick the first one
            return Ok(device);
        }

        Err(GraphicsError::DeviceInitializationFailed(
            "No Vulkan-compatible GPU found".into(),
        ))
    }

    fn create_logical_device(
        instance: &ash::Instance,
        physical_device: vk::PhysicalDevice,
    ) -> Result<(ash::Device, vk::Queue), GraphicsError> {
        let queue_family_properties =
            unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

        let mut graphics_family_index = None;

        for (index, info) in queue_family_properties.iter().enumerate() {
            if info.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                graphics_family_index = Some(index as u32);
                break;
            }
        }

        let graphics_family_index = graphics_family_index.ok_or(
            GraphicsError::DeviceInitializationFailed("No graphics queue found".into()),
        )?;

        let queue_priority = [1.0_f32];
        let queue_info = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(graphics_family_index)
            .queue_priorities(&queue_priority)];

        let device_features = vk::PhysicalDeviceFeatures::default();

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_info)
            .enabled_features(&device_features);

        let device = unsafe {
            instance
                .create_device(physical_device, &device_create_info, None)
                .map_err(|e| {
                    GraphicsError::DeviceInitializationFailed(format!(
                        "Failed to create logical device: {:?}",
                        e
                    ))
                })?
        };

        let graphics_queue = unsafe { device.get_device_queue(graphics_family_index, 0) };

        Ok((device, graphics_queue))
    }
}

impl Drop for VulkanGraphicsDevice {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_device(None);

            if let (Some(utils), Some(messenger)) =
                (self.debug_utils.as_ref(), self.debug_messenger)
            {
                utils.destroy_debug_utils_messenger(messenger, None);
            }

            self.instance.destroy_instance(None);
        }
    }
}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let callback_data = unsafe { *p_callback_data };
    let message = if callback_data.p_message.is_null() {
        String::from("No message")
    } else {
        unsafe {
            CStr::from_ptr(callback_data.p_message)
                .to_string_lossy()
                .into_owned()
        }
    };

    let severity = match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => "ERROR",
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => "WARNING",
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => "INFO",
        _ => "VERBOSE",
    };

    let msg_type = match message_type {
        vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "GENERAL",
        vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "VALIDATION",
        vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "PERFORMANCE",
        _ => "UNKNOWN",
    };

    eprintln!("[Vulkan {} {}] {}", severity, msg_type, message);

    vk::FALSE
}
