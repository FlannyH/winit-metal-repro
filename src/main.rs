use std::{mem, sync::Arc};

use glam::{Vec2, Vec3};
use metal::{
    DeviceRef, LibraryRef, MTLClearColor, MTLLoadAction, MTLPixelFormat, MTLPrimitiveType,
    MTLResourceOptions, MTLScissorRect, MTLStoreAction, RenderPassDescriptor,
    RenderPipelineDescriptor, RenderPipelineState,
};
use render_loop::{RenderLoop, RenderLoopCreateDesc};

mod event_source;
mod render_loop;
mod swapchain;

fn main() {
    RenderLoop::spawn_render_thread(
        RenderLoopCreateDesc {
            window_title: "Winit MacOS test app",
            window_size: (1280, 720),
        },
        move |mut event_source| {
            let device_arc = Arc::new(metal::Device::system_default().unwrap());
            let device = device_arc.as_ref();

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
                MTLResourceOptions::CPUCacheModeDefaultCache
                    | MTLResourceOptions::StorageModeManaged,
            );

            let mut sw_size = event_source.early_window_size();

            loop {
                let event = event_source.recv(&device_arc).unwrap();

                let swapchain = match event {
                    event_source::EventSourceEvent::Terminate => break,
                    event_source::EventSourceEvent::Render(swapchain) => swapchain,
                    event_source::EventSourceEvent::Event(_) => continue,
                    event_source::EventSourceEvent::WindowEvent(_) => continue,
                }
                .unwrap();

                let swapchain_size = swapchain.layer.drawable_size();
                let swapchain_size = (swapchain_size.width as u32, swapchain_size.height as u32);

                if sw_size != swapchain_size {
                    println!("Resize {:?} => {:?}", sw_size, swapchain_size);
                    sw_size = swapchain_size;
                }

                let drawable = swapchain
                    .layer
                    .next_drawable()
                    .expect("Failed to get drawable");

                // Set up framebuffer
                let render_pass_descriptor = RenderPassDescriptor::new();
                let color_attachment = render_pass_descriptor
                    .color_attachments()
                    .object_at(0)
                    .unwrap();
                color_attachment.set_texture(Some(drawable.texture()));
                color_attachment.set_load_action(MTLLoadAction::Clear);
                color_attachment.set_clear_color(MTLClearColor::new(1.0, 0.2, 0.25, 1.0));
                color_attachment.set_store_action(MTLStoreAction::Store);

                // Set up command buffer
                let command_buffer = command_queue.new_command_buffer();
                let command_encoder =
                    command_buffer.new_render_command_encoder(&render_pass_descriptor);

                // Record triangle draw call
                println!("{}", vertex_buffer_data.len());
                println!("{}", vertex_buffer.length());
                println!("{}", vertex_buffer.allocated_size());
                unsafe {
                    //&*(vertex_buffer.contents() as *const HelloTriangleVertex)
                    println!(
                        "{:?}",
                        &*(vertex_buffer.contents() as *const HelloTriangleVertex)
                    );
                    println!(
                        "{:?}",
                        *((vertex_buffer.contents() as *mut HelloTriangleVertex).wrapping_add(1))
                    );
                    println!(
                        "{:?}",
                        *((vertex_buffer.contents() as *mut HelloTriangleVertex).wrapping_add(2))
                    );
                }
                command_encoder.set_render_pipeline_state(&hello_triangle_pipeline_state);
                command_encoder.set_scissor_rect(MTLScissorRect {
                    x: 0,
                    y: 0,
                    width: sw_size.0 as u64,
                    height: sw_size.1 as u64,
                });
                command_encoder.set_vertex_buffer(0, Some(&vertex_buffer), 0);
                command_encoder.draw_primitives(
                    MTLPrimitiveType::Triangle,
                    0,
                    vertex_buffer_data.len() as u64,
                );
                command_encoder.end_encoding();
                command_buffer.commit();
            }
        },
    )
    .run_loop();
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

#[repr(C)]
#[derive(Debug)]
struct HelloTriangleVertex {
    position: Vec2,
    color: Vec3,
}
