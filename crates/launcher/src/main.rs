use graphics::vulkan::device::VulkanGraphicsDevice;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

#[derive(Default)]
struct App {
    window: Option<Window>,
    vulkan_device: Option<VulkanGraphicsDevice>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window_attributes = Window::default_attributes()
            .with_title("Untitled")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

        let window = event_loop
            .create_window(window_attributes)
            .expect("Failed to create window");

        self.vulkan_device = Some(
            VulkanGraphicsDevice::new(&window).expect("Failed to create VulkanGraphicsDevice"),
        );

        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                if let Some(device) = &self.vulkan_device {
                    let _ = device.wait_idle();
                }
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // Draw frame
                if let Some(device) = &mut self.vulkan_device {
                    if let Err(e) = device.draw_frame() {
                        eprintln!("Failed to draw frame: {}", e);
                    }
                }

                // Request next redraw
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        // Request redraw on each event loop iteration
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;

    // Poll mode for continuous rendering
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}
