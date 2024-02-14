use cocoa::appkit::NSView;
use core_graphics_types::geometry::CGSize;
use foreign_types_shared::ForeignType;
use metal::{MTLPixelFormat, MetalLayer};
use objc::runtime::YES;
use raw_window_handle::HasRawWindowHandle;

pub struct Swapchain {
    pub layer: metal::MetalLayer,
}

impl Swapchain {
    pub fn new(
        device: &metal::Device,
        window_handle: &dyn HasRawWindowHandle,
        width: u64,
        height: u64,
    ) -> Self {
        let layer = MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::RGBA8Unorm);
        layer.set_presents_with_transaction(false);
        layer.set_drawable_size(CGSize::new(width as f64, height as f64));

        let handle = match window_handle.raw_window_handle() {
            raw_window_handle::RawWindowHandle::AppKit(handle) => handle,
            x => panic!("Expected AppKit handle in Metal swapchain creation, got: {x:?}"),
        };

        unsafe {
            let view = handle.ns_view as cocoa::base::id;
            view.setWantsLayer(YES);
            view.setLayer(layer.as_ptr().cast());
        }

        Self { layer }
    }
}
