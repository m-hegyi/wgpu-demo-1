use glyphon::{
    Attrs, Buffer, Color, FontSystem, Metrics, Resolution, Shaping, SwashCache, TextArea,
    TextAtlas, TextBounds, TextRenderer,
};
use wgpu::MultisampleState;

pub struct Text {
    text_renderer: TextRenderer,
    pub buffer: Buffer,
    pub font_system: FontSystem,
    atlas: TextAtlas,
    cache: SwashCache,
}

impl Text {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        swapchain_format: wgpu::TextureFormat,
    ) -> Self {
        let mut font_system = FontSystem::new();
        let cache = SwashCache::new();
        let mut atlas = TextAtlas::new(device, queue, swapchain_format);
        let text_renderer =
            TextRenderer::new(&mut atlas, &device, MultisampleState::default(), None);

        let buffer = Buffer::new(&mut font_system, Metrics::new(30.0, 42.0));

        Text {
            font_system,
            cache,
            atlas,
            text_renderer,
            buffer,
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_resolution: Resolution,
    ) {
        let _ = self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            screen_resolution,
            [TextArea {
                buffer: &self.buffer,
                left: 10.0,
                top: 10.0,
                scale: 1.0,
                bounds: TextBounds {
                    left: 0,
                    top: 0,
                    right: 600,
                    bottom: 160,
                },
                default_color: Color::rgb(255, 255, 255),
            }],
            &mut self.cache,
        );
    }

    pub fn render<'rpass>(&'rpass self, render_pass: &mut wgpu::RenderPass<'rpass>) {
        let _ = self.text_renderer.render(&self.atlas, render_pass).unwrap();
    }

    pub fn set_text(&mut self, text: &str) {
        self.buffer.set_text(
            &mut self.font_system,
            text,
            Attrs::new().family(glyphon::Family::SansSerif),
            Shaping::Advanced,
        );
    }
}
