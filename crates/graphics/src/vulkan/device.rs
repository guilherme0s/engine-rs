use ash::{
    khr,
    vk::{self},
};
use std::ffi::{CStr, CString, c_char};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};

const MAX_FRAMES_IN_FLIGHT: u32 = 3;

pub struct VulkanGraphicsDevice {
    instance: ash::Instance,
    debug_utils_loader: ash::ext::debug_utils::Instance,
    debug_messenger: vk::DebugUtilsMessengerEXT,
    surface_loader: khr::surface::Instance,
    surface: vk::SurfaceKHR,
    _physical_device: vk::PhysicalDevice,
    device: ash::Device,
    _graphics_queue: vk::Queue,
    _graphics_family_index: u32,
    swapchain_loader: khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_image_views: Vec<vk::ImageView>,
    image_available_semaphores: Vec<vk::Semaphore>,
    render_finished_semaphores: Vec<vk::Semaphore>,
    in_flight_fences: Vec<vk::Fence>,
    render_pass: vk::RenderPass,
}

impl VulkanGraphicsDevice {
    pub fn new(window: &winit::window::Window) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = unsafe { ash::Entry::load()? };
        let instance = Self::create_instance(&entry)?;

        let (debug_utils_loader, debug_messenger) = Self::setup_debug_messenger(&entry, &instance)?;

        let surface_loader = khr::surface::Instance::new(&entry, &instance);
        let surface = unsafe {
            ash_window::create_surface(
                &entry,
                &instance,
                window.display_handle().unwrap().as_raw(),
                window.window_handle().unwrap().as_raw(),
                None,
            )?
        };

        let physical_device = Self::select_physical_device(&instance)?;
        let (device, graphics_queue, graphics_family_index) =
            Self::create_logical_device(&instance, physical_device)?;

        let swapchain_loader = khr::swapchain::Device::new(&instance, &device);
        let (swapchain, swapchain_images, swapchain_format) = Self::create_swapchain(
            physical_device,
            &surface_loader,
            &swapchain_loader,
            surface,
            window.inner_size().width,
            window.inner_size().height,
        )?;
        let swapchain_image_views =
            Self::create_image_views(&device, &swapchain_images, swapchain_format)?;

        let render_pass = Self::create_render_pass(&device, swapchain_format)?;

        let (image_available_semaphores, render_finished_semaphores, in_flight_fences) =
            Self::create_sync_objects(&device)?;

        Ok(Self {
            instance,
            debug_utils_loader,
            debug_messenger,
            surface_loader,
            surface,
            _physical_device: physical_device,
            device,
            _graphics_queue: graphics_queue,
            _graphics_family_index: graphics_family_index,
            swapchain_loader,
            swapchain,
            swapchain_image_views,
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            render_pass,
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
            } else if a_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
                std::cmp::Ordering::Less
            } else if b_type == vk::PhysicalDeviceType::INTEGRATED_GPU {
                std::cmp::Ordering::Greater
            } else {
                a.index.cmp(&b.index)
            }
        });

        device_infos
            .first()
            .map(|info| Ok(info.handle))
            .unwrap_or(Err("No suitable GPU found".into()))
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

    fn create_swapchain(
        physical_device: vk::PhysicalDevice,
        surface_loader: &khr::surface::Instance,
        swapchain_loader: &khr::swapchain::Device,
        surface: vk::SurfaceKHR,
        width: u32,
        height: u32,
    ) -> Result<(vk::SwapchainKHR, Vec<vk::Image>, vk::Format), Box<dyn std::error::Error>> {
        let surface_capabilities = unsafe {
            surface_loader.get_physical_device_surface_capabilities(physical_device, surface)?
        };

        let surface_formats = unsafe {
            surface_loader.get_physical_device_surface_formats(physical_device, surface)?
        };

        let surface_format = surface_formats
            .iter()
            .find(|f| {
                f.format == vk::Format::B8G8R8A8_SRGB
                    && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
            })
            .unwrap_or(&surface_formats[0]);

        let extent = if surface_capabilities.current_extent.width != u32::MAX {
            surface_capabilities.current_extent
        } else {
            vk::Extent2D {
                width: width.clamp(
                    surface_capabilities.min_image_extent.width,
                    surface_capabilities.max_image_extent.width,
                ),
                height: height.clamp(
                    surface_capabilities.min_image_extent.height,
                    surface_capabilities.max_image_extent.height,
                ),
            }
        };

        let image_count = (surface_capabilities.min_image_count + 1).min(
            if surface_capabilities.max_image_count > 0 {
                surface_capabilities.max_image_count
            } else {
                u32::MAX
            },
        );

        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::FIFO)
            .clipped(true);

        let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None)? };
        let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };

        Ok((swapchain, swapchain_images, surface_format.format))
    }

    fn create_image_views(
        device: &ash::Device,
        images: &[vk::Image],
        format: vk::Format,
    ) -> Result<Vec<vk::ImageView>, Box<dyn std::error::Error>> {
        let mut image_views = Vec::new();

        for &image in images {
            let create_info = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format)
                .components(vk::ComponentMapping {
                    r: vk::ComponentSwizzle::IDENTITY,
                    g: vk::ComponentSwizzle::IDENTITY,
                    b: vk::ComponentSwizzle::IDENTITY,
                    a: vk::ComponentSwizzle::IDENTITY,
                })
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            let image_view = unsafe { device.create_image_view(&create_info, None)? };

            image_views.push(image_view);
        }

        Ok(image_views)
    }

    fn create_render_pass(
        device: &ash::Device,
        format: vk::Format,
    ) -> Result<vk::RenderPass, Box<dyn std::error::Error>> {
        let color_attachment = vk::AttachmentDescription::default()
            .format(format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let color_attachment_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_attachment_ref));

        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);

        let render_pass_info = vk::RenderPassCreateInfo::default()
            .attachments(std::slice::from_ref(&color_attachment))
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&dependency));

        let render_pass = unsafe { device.create_render_pass(&render_pass_info, None)? };
        Ok(render_pass)
    }

    fn create_sync_objects(
        device: &ash::Device,
    ) -> Result<(Vec<vk::Semaphore>, Vec<vk::Semaphore>, Vec<vk::Fence>), Box<dyn std::error::Error>>
    {
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED);

        let mut image_available = Vec::new();
        let mut render_finished = Vec::new();
        let mut in_flight = Vec::new();

        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            let image_sem = unsafe { device.create_semaphore(&semaphore_info, None)? };
            let render_sem = unsafe { device.create_semaphore(&semaphore_info, None)? };
            let fence = unsafe { device.create_fence(&fence_info, None)? };

            image_available.push(image_sem);
            render_finished.push(render_sem);
            in_flight.push(fence);
        }

        Ok((image_available, render_finished, in_flight))
    }
}

impl Drop for VulkanGraphicsDevice {
    fn drop(&mut self) {
        unsafe {
            // Wait for device to finish all operations before destroying
            let _ = self.device.device_wait_idle();

            for &semaphore in &self.image_available_semaphores {
                self.device.destroy_semaphore(semaphore, None);
            }
            for &semaphore in &self.render_finished_semaphores {
                self.device.destroy_semaphore(semaphore, None);
            }
            for &fence in &self.in_flight_fences {
                self.device.destroy_fence(fence, None);
            }

            self.device.destroy_render_pass(self.render_pass, None);
            for &image_view in &self.swapchain_image_views {
                self.device.destroy_image_view(image_view, None);
            }

            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);

            self.device.destroy_device(None);

            self.surface_loader.destroy_surface(self.surface, None);

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
