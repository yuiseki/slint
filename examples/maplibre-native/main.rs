// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use slint::wgpu_24::{wgpu, WGPUConfiguration, WGPUSettings};

struct MapRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    displayed_texture: wgpu::Texture,
    next_texture: wgpu::Texture,
    start_time: std::time::Instant,
    
    // Map state
    latitude: f32,
    longitude: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MapData {
    viewport: [f32; 4],  // lat, lng, zoom, time
    pan_offset: [f32; 2], // pan offset
    _padding: [f32; 2],   // Ensure 16-byte alignment
}

impl MapRenderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Map Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "map_shader.wgsl"
            ))),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Map Pipeline Layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX_FRAGMENT,
                range: 0..32, // MapData struct size in bytes
            }],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Map Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let displayed_texture = Self::create_texture(&device, 512, 512);
        let next_texture = Self::create_texture(&device, 512, 512);

        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            displayed_texture,
            next_texture,
            start_time: std::time::Instant::now(),
            latitude: 35.6762,   // Tokyo
            longitude: 139.6503,
            zoom: 10.0,
            pan_x: 0.0,
            pan_y: 0.0,
        }
    }

    fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Map Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn update_viewport(&mut self, lat: f32, lng: f32, zoom: f32) {
        self.latitude = lat;
        self.longitude = lng;
        self.zoom = zoom;
    }

    fn pan(&mut self, dx: f32, dy: f32) {
        // Scale pan amount by zoom level
        let scale = 1.0 / self.zoom;
        self.pan_x += dx * scale;
        self.pan_y += dy * scale;
    }

    fn reset_view(&mut self) {
        self.latitude = 35.6762;
        self.longitude = 139.6503;
        self.zoom = 10.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
    }

    fn render(&mut self, width: u32, height: u32) -> wgpu::Texture {
        if self.next_texture.size().width != width || self.next_texture.size().height != height {
            let mut new_texture = Self::create_texture(&self.device, width, height);
            std::mem::swap(&mut self.next_texture, &mut new_texture);
        }

        let elapsed: f32 = self.start_time.elapsed().as_millis() as f32 / 1000.0;
        let map_data = MapData {
            viewport: [self.latitude, self.longitude, self.zoom, elapsed],
            pan_offset: [self.pan_x, self.pan_y],
            _padding: [0.0, 0.0],
        };

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { 
            label: Some("Map Render Encoder") 
        });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Map Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.next_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.2, g: 0.4, b: 0.8, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_push_constants(
                wgpu::ShaderStages::VERTEX_FRAGMENT,
                0,
                bytemuck::bytes_of(&map_data),
            );
            rpass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));

        let result_texture = self.next_texture.clone();
        std::mem::swap(&mut self.next_texture, &mut self.displayed_texture);

        result_texture
    }
}

fn main() {
    let mut wgpu_settings = WGPUSettings::default();
    wgpu_settings.device_required_features = wgpu::Features::PUSH_CONSTANTS;
    wgpu_settings.device_required_limits.max_push_constant_size = 32;

    slint::BackendSelector::new()
        .require_wgpu_24(WGPUConfiguration::Automatic(wgpu_settings))
        .select()
        .expect("Unable to create Slint backend with WGPU renderer");

    let app = MapLibreDemo::new().unwrap();
    let mut map_renderer = None;
    let app_weak = app.as_weak();

    // Set up map controls
    let app_weak_pan = app_weak.clone();
    app.on_pan_map(move |dx, dy| {
        // Note: We'll handle the pan in the renderer
        if let Some(app) = app_weak_pan.upgrade() {
            app.window().request_redraw();
        }
    });

    let app_weak_zoom = app_weak.clone();
    app.on_zoom_changed(move |_zoom| {
        if let Some(app) = app_weak_zoom.upgrade() {
            app.window().request_redraw();
        }
    });

    let app_weak_reset = app_weak.clone();
    app.on_reset_view(move || {
        if let Some(app) = app_weak_reset.upgrade() {
            app.set_latitude(35.6762);
            app.set_longitude(139.6503);
            app.set_zoom_level(10.0);
            app.window().request_redraw();
        }
    });

    let app_weak_redraw = app_weak.clone();
    app.on_request_redraw(move || {
        if let Some(app) = app_weak_redraw.upgrade() {
            app.window().request_redraw();
        }
    });

    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            match state {
                slint::RenderingState::RenderingSetup => {
                    match graphics_api {
                        slint::GraphicsAPI::WGPU24 { device, queue, .. } => {
                            map_renderer = Some(MapRenderer::new(device, queue));
                        }
                        _ => return,
                    };
                }
                slint::RenderingState::BeforeRendering => {
                    if let (Some(renderer), Some(app)) = (map_renderer.as_mut(), app_weak.upgrade()) {
                        // Update map state
                        renderer.update_viewport(
                            app.get_latitude(),
                            app.get_longitude(),
                            app.get_zoom_level(),
                        );

                        // Render map to texture
                        let texture = renderer.render(512, 512);
                        app.set_rendered_map(slint::Image::try_from(texture).unwrap());
                    }
                }
                slint::RenderingState::AfterRendering => {}
                slint::RenderingState::RenderingTeardown => {
                    drop(map_renderer.take());
                }
                _ => {}
            }
        })
        .expect("Unable to set rendering notifier");

    app.run().unwrap();
}