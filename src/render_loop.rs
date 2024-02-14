use std::{
    panic::resume_unwind,
    sync::Arc,
    thread::{Builder, JoinHandle},
};

use crossbeam_channel::Sender;
use once_cell::sync::OnceCell;
use winit::{
    dpi::PhysicalSize,
    event, event_loop,
    platform::run_on_demand::EventLoopExtRunOnDemand,
    window::{Window, WindowBuilder},
};

use crate::event_source::EventSource;

#[derive(Debug)]
pub struct EarlyWindowData {
    pub window_size: (u32, u32),
    pub winit_window: Option<Arc<Window>>,
}

pub struct RenderLoopCreateDesc {
    pub window_title: &'static str,
    pub window_size: (u32, u32),
}

pub struct RenderLoop<ThreadResult: Send + 'static> {
    create_desc: RenderLoopCreateDesc,
    join_handle: JoinHandle<ThreadResult>,
    event_send: Sender<event::Event<()>>,
    early_window_data: Arc<OnceCell<EarlyWindowData>>,
}

impl<ThreadResult: Send + 'static> RenderLoop<ThreadResult> {
    pub fn spawn_render_thread(
        create_desc: RenderLoopCreateDesc,
        thread_fn: impl FnOnce(EventSource) -> ThreadResult + Send + 'static,
    ) -> Self {
        let (event_send, event_recv) = crossbeam_channel::bounded(1);

        let early_window_data = Arc::new(OnceCell::new());
        let early_window_data_thread = early_window_data.clone();

        let join_handle = Builder::new()
            .name("Render Loop".to_string())
            .spawn(move || {
                thread_fn(EventSource {
                    event_recv,
                    swapchain: None,
                    early_window_data: early_window_data_thread,
                })
            })
            .expect("Failed to start thread");

        Self {
            join_handle,
            create_desc,
            event_send,
            early_window_data,
        }
    }

    pub fn run_loop(self) -> ThreadResult {
        let mut event_loop = event_loop::EventLoop::new().unwrap();
        let window_size = PhysicalSize::new(
            self.create_desc.window_size.0,
            self.create_desc.window_size.1,
        );
        let window = Arc::new(
            WindowBuilder::new()
                .with_title("")
                .with_inner_size(window_size)
                .build(&event_loop)
                .unwrap(),
        );
        self.early_window_data
            .set(EarlyWindowData {
                window_size: (window_size.width, window_size.height),
                winit_window: Some(window.clone()),
            })
            .expect("Window data already set!");

        let event_send = self.event_send.clone();

        let mut has_window = true;
        let _ = event_loop.run_on_demand(move |event, elwt| {
            match event {
                event::Event::Suspended => {
                    has_window = false;
                    println!("Pausing rendering");
                }
                event::Event::Resumed => {
                    has_window = true;
                    println!("Resuming rendering");
                }
                event::Event::WindowEvent { window_id, .. } => {
                    assert_eq!(window_id, window.id(), "Multi-window not supported!");
                }
                _ => {}
            }

            elwt.set_control_flow(if has_window {
                event_loop::ControlFlow::Poll
            } else {
                event_loop::ControlFlow::Wait
            });

            match event {
                event::Event::WindowEvent {
                    event: event::WindowEvent::ScaleFactorChanged { .. },
                    ..
                } => {}
                event => {
                    if event_send.send(event).is_err() {
                        elwt.exit();
                    }
                }
            }
        });

        self.wait_for_exit()
    }

    pub fn wait_for_exit(self) -> ThreadResult {
        match self.join_handle.join() {
            Ok(v) => v,
            Err(err) => {
                let msg = err
                    .downcast_ref::<&'static str>()
                    .copied()
                    .or_else(|| err.downcast_ref::<String>().map(|v| v.as_str()))
                    .unwrap_or("Unknown boxed error type");
                println!("RenderLoop thread panicked: {}", msg);
                resume_unwind(err)
            }
        }
    }
}
