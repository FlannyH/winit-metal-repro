use std::sync::Arc;

use crossbeam_channel::Receiver;
use winit::event::{self, Event};

use crate::{render_loop::EarlyWindowData, swapchain::Swapchain};
use once_cell::sync::OnceCell;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Disconnected;

#[allow(dead_code)]
pub enum EventSourceEvent {
    Terminate,
    Render(Option<Arc<Swapchain>>),
    Event(event::Event<()>),
    WindowEvent(event::WindowEvent),
}

pub struct EventSource {
    pub event_recv: Receiver<event::Event<()>>,
    pub early_window_data: Arc<OnceCell<EarlyWindowData>>,
    pub swapchain: Option<Arc<Swapchain>>,
}

impl EventSource {
    pub fn recv(&mut self, device: &Arc<metal::Device>) -> Result<EventSourceEvent, Disconnected> {
        let event = self.event_recv.recv().map_err(|_recv_error| Disconnected)?;

        Ok(match event {
            Event::WindowEvent { event, .. } => match event {
                event::WindowEvent::Resized(..) => {
                    // resize swapchain
                    todo!()
                }
                event::WindowEvent::CloseRequested => {
                    // terminate
                    todo!()
                }
                event::WindowEvent::KeyboardInput { .. } => {
                    // if escape, terminate
                    todo!()
                }
                x => EventSourceEvent::WindowEvent(x),
            },
            Event::Suspended => {
                // destroy swapchain
                todo!()
            }
            Event::Resumed => {
                let ewd = self.early_window_data.wait();
                let window = ewd.winit_window.as_ref().unwrap();
                let size = window.inner_size();
                println!("Resumed: Creating swapchain {}×{}", size.width, size.height);
                assert!(
                    self.swapchain.is_none(),
                    "Unbalanced Resumed event, app is already running"
                );
                self.swapchain = Some(Arc::new(Swapchain::new(
                    &device,
                    window.as_ref(),
                    size.width.into(),
                    size.height.into(),
                )));
                EventSourceEvent::Event(event)
            }
            Event::AboutToWait => EventSourceEvent::Render(self.swapchain.clone()),
            x => EventSourceEvent::Event(x),
        })
    }

    pub(crate) fn early_window_size(&self) -> (u32, u32) {
        self.early_window_data.wait().window_size
    }
}
