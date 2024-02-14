use std::{cell::OnceCell, mem, sync::Arc};

use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use crossbeam_channel::{Receiver, RecvError, Sender};
use glam::{Vec2, Vec3};
use metal::{
    objc::runtime::YES, Device, DeviceRef, LibraryRef, MTLPixelFormat, MTLResourceOptions,
    MetalLayer, RenderPipelineDescriptor, RenderPipelineState,
};
use winit::{
    dpi::PhysicalSize,
    event,
    event_loop::{self, EventLoop},
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::{Window, WindowBuilder},
};

#[repr(C)]
#[derive(Debug)]
struct HelloTriangleVertex {
    position: Vec2,
    color: Vec3,
}

pub struct EarlyWindowData {
    window_size: (u32, u32),
    winit_window: Option<Arc<Window>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Disconnected;

pub enum Event<'swc> {
    Terminate,
    /// Users should handle "resize" events by looking at [`Swapchain::size()`].
    /// (This is implicit for the Render Graph).
    Render(Option<SwapchainForRendering<'swc>>),
    /// Contains the current window size
    // TODO(Marijn): Remove this when breda-egui goes through our internal
    // input handling system; or breda-egui is coordinated from this crate.
    TempWinitForInput(event::Event<()>, (u32, u32)),
}
struct EventSource {
    event_recv: Receiver<EventLoopEvent>,
    early_window_data: Arc<OnceCell<EarlyWindowData>>,
}

impl EventSource {
    pub fn recv(&mut self, device: &metal::Device) -> Result<Event<'_>, Disconnected> {
        let event = self.event_recv.recv().map_err(|RecvError| Disconnected)?;
    }
}

/// Event originating from the event loop, intended for the (private) event handler
/// inside [`EventSource`].
#[derive(Debug)]
enum EventLoopEvent {
    Winit(event::Event<()>),
}

fn main() {
    let (event_send, event_recv) = crossbeam_channel::bounded(1);

    let join_handle = std::thread::Builder::new()
        .name("Render Loop".to_string())
        .spawn(move || {});
    create_window_and_event_loop(event_send);
}

fn create_window_and_event_loop(event_send: Sender<EventLoopEvent>) {
    let mut event_loop = EventLoop::new().unwrap();
    let window_size = PhysicalSize::new(1280, 720);
    let window = WindowBuilder::new()
        .with_title("Test")
        .with_inner_size(window_size)
        .with_resizable(true)
        .build(&event_loop)
        .unwrap();

    let mut has_window = true;

    event_loop
        .run_on_demand(move |event, elwt| {
            match event {
                event::Event::Suspended => {
                    has_window = false;
                    println!("Pausing rendering");
                }
                event::Event::Resumed => {
                    has_window = true;
                    println!("Resuming rendering");
                }
                _ => {}
            }
            elwt.set_control_flow(if has_window {
                event_loop::ControlFlow::Poll
            } else {
                // If we're not rendering, wait for the next event instead of looping endlessly,
                // the application isn't visible anyway.
                event_loop::ControlFlow::Wait
            });

            match event {
                event::Event::WindowEvent {
                    event: event::WindowEvent::ScaleFactorChanged { .. },
                    ..
                } => {}
                event => {
                    if event_send.send(EventLoopEvent::Winit(event)).is_err() {
                        elwt.exit();
                    }
                }
            }
        })
        .unwrap();
}

fn prepare_pipeline_state(
    device: &DeviceRef,
    library: &LibraryRef,
    vertex_shader_path: &str,
    fragment_shader_path: &str,
) -> RenderPipelineState {
    // Get compiled functions from the library
    let vertex_function = library.get_function(vertex_shader_path, None).unwrap();
    let fragment_function = library.get_function(fragment_shader_path, None).unwrap();

    // Create pipeline state descriptor - handles things like shader program, buffer to render to, blend mode, etc.
    let pipeline_state_desc = RenderPipelineDescriptor::new();
    pipeline_state_desc.set_vertex_function(Some(&vertex_function));
    pipeline_state_desc.set_fragment_function(Some(&fragment_function));

    let attachment = pipeline_state_desc
        .color_attachments()
        .object_at(0)
        .unwrap();
    attachment.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
    attachment.set_blending_enabled(false);
    return device
        .new_render_pipeline_state(&pipeline_state_desc)
        .unwrap();
}

fn render_loop(event_source: EventSource) {
    // Create device
    let device = Device::system_default().expect("Could not create device.");

    // Create metal layer
    let layer = MetalLayer::new();
    layer.set_device(&device);
    layer.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
    layer.set_presents_with_transaction(false);

    // Create view - a sort of canvas where you draw graphics using Metal commands
    let window = event_source
        .early_window_data
        .as_ref()
        .get()
        .unwrap()
        .winit_window
        .unwrap();

    unsafe {
        let view = window.ns_view() as cocoa_id;
        view.setWantsLayer(YES);
        view.setLayer(mem::transmute(layer.as_ref()));
    }

    let drawable_size = window.inner_size();
    layer.set_drawable_size(CGSize::new(
        drawable_size.width as f64,
        drawable_size.height as f64,
    ));

    // Load the Metal library file
    let compile_options = metal::CompileOptions::new();
    let library = device
        .new_library_with_source(include_str!("../assets/triangle.metal"), &compile_options)
        .expect("Failed to load Metal library");

    // Initialize the pipeline states with functions from this library
    let hello_triangle_pipeline_state = prepare_pipeline_state(
        &device,
        &library,
        "hello_triangle_vertex",
        "hello_triangle_fragment",
    );

    // Create command queue
    let command_queue = device.new_command_queue();

    // Set up vertex buffer data for the triangle
    let mut vertex_buffer_data = Vec::<HelloTriangleVertex>::new();
    // todo: make sure the winding order is correct
    vertex_buffer_data.push(HelloTriangleVertex {
        position: Vec2 { x: -0.5, y: -0.5 },
        color: Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
    });
    vertex_buffer_data.push(HelloTriangleVertex {
        position: Vec2 { x: 0.5, y: -0.5 },
        color: Vec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
    });
    vertex_buffer_data.push(HelloTriangleVertex {
        position: Vec2 { x: 0.0, y: 0.5 },
        color: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        },
    });

    // Create the vertex buffer on the device
    let vertex_buffer = device.new_buffer_with_data(
        vertex_buffer_data.as_ptr() as *const _,
        (vertex_buffer_data.len() * mem::size_of::<HelloTriangleVertex>()) as u64,
        MTLResourceOptions::CPUCacheModeDefaultCache | MTLResourceOptions::StorageModeManaged,
    );
}
