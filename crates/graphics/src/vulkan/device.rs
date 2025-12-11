use std::ffi::{CStr, CString};

use ash::{khr, vk};

use crate::error::GraphicsError;

pub struct VulkanGraphicsDevice {
    _entry: ash::Entry,
    instance: ash::Instance,
    #[cfg(debug_assertions)]
    debug_utils: Option<ash::ext::debug_utils::Instance>,
    #[cfg(debug_assertions)]
    debug_messenger: Option<vk::DebugUtilsMessengerEXT>,
}

impl VulkanGraphicsDevice {
    pub fn new() -> Result<Self, GraphicsError> {
        let entry = unsafe {
            ash::Entry::load().map_err(|e| {
                GraphicsError::DeviceInitializationFailed(format!("Failed to load Vulkan: {:?}", e))
            })?
        };
        let (instance, debug_utils, debug_messenger) = Self::create_instance(&entry)?;

        Ok(Self {
            _entry: entry,
            instance,
            debug_messenger,
            debug_utils,
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
}

impl Drop for VulkanGraphicsDevice {
    fn drop(&mut self) {
        unsafe {
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
