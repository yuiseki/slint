// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use slint::wgpu_24::{wgpu, WGPUConfiguration, WGPUSettings};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Deserialize)]
struct VectorTileSource {
    tiles: Vec<String>,
    minzoom: Option<u8>,
    maxzoom: Option<u8>,
}

#[derive(Debug, Deserialize)]
struct MapStyle {
    sources: std::collections::HashMap<String, serde_json::Value>,
}

struct VectorTileLoader {
    client: reqwest::Client,
    tile_cache: Arc<Mutex<std::collections::HashMap<String, wgpu::Texture>>>,
}

impl VectorTileLoader {
    fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            tile_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    fn get_tile_coordinates(lat: f64, lng: f64, zoom: u8) -> (u32, u32) {
        let n = 2.0_f64.powi(zoom as i32);
        let x = ((lng + 180.0) / 360.0 * n).floor() as u32;
        let y = ((1.0 - (lat.to_radians().tan() + 1.0 / lat.to_radians().cos()).ln() / std::f64::consts::PI) / 2.0 * n).floor() as u32;
        (x, y)
    }

    async fn fetch_tile_image(&self, x: u32, y: u32, z: u8) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Use OpenStreetMap tiles as raster fallback for vector tiles
        let url = format!("https://tile.openstreetmap.org/{}/{}/{}.png", z, x, y);
        
        let response = self.client.get(&url)
            .header("User-Agent", "Slint MapLibre Example/1.0")
            .send()
            .await?;

        if response.status().is_success() {
            let bytes = response.bytes().await?;
            Ok(bytes.to_vec())
        } else {
            Err(format!("HTTP error: {}", response.status()).into())
        }
    }

    async fn load_tile_texture(&self, device: &wgpu::Device, queue: &wgpu::Queue, lat: f64, lng: f64, zoom: u8) -> Option<wgpu::Texture> {
        let (x, y) = Self::get_tile_coordinates(lat, lng, zoom);
        let cache_key = format!("{}/{}/{}", zoom, x, y);

        // Check cache first
        {
            let cache = self.tile_cache.lock().await;
            if let Some(texture) = cache.get(&cache_key) {
                return Some(texture.clone());
            }
        }

        // Fetch tile image
        if let Ok(image_data) = self.fetch_tile_image(x, y, zoom).await {
            if let Ok(img) = image::load_from_memory(&image_data) {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();

                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(&format!("Tile {}/{}/{}", zoom, x, y)),
                    size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * width),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                );

                // Cache the texture
                {
                    let mut cache = self.tile_cache.lock().await;
                    cache.insert(cache_key, texture.clone());
                }

                return Some(texture);
            }
        }

        None
    }
}

struct MapRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    displayed_texture: wgpu::Texture,
    next_texture: wgpu::Texture,
    tile_loader: VectorTileLoader,
    start_time: std::time::Instant,
    
    // Map state
    latitude: f32,
    longitude: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,

    // Current tile texture and bind group
    current_tile_texture: Option<wgpu::Texture>,
    current_bind_group: Option<wgpu::BindGroup>,
    sampler: wgpu::Sampler,
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

        // Create bind group layout for textures
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Tile Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Map Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Tile Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let displayed_texture = Self::create_texture(&device, 512, 512);
        let next_texture = Self::create_texture(&device, 512, 512);

        Self {
            device: device.clone(),
            queue: queue.clone(),
            pipeline,
            bind_group_layout,
            displayed_texture,
            next_texture,
            tile_loader: VectorTileLoader::new(),
            start_time: std::time::Instant::now(),
            latitude: 35.6762,   // Tokyo
            longitude: 139.6503,
            zoom: 10.0,
            pan_x: 0.0,
            pan_y: 0.0,
            current_tile_texture: None,
            current_bind_group: None,
            sampler,
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
        let scale = 1.0 / self.zoom;
        self.pan_x += dx * scale;
        self.pan_y += dy * scale;
    }

    async fn update_tile_texture(&mut self) {
        let zoom_level = self.zoom.clamp(1.0, 18.0) as u8;
        
        if let Some(tile_texture) = self.tile_loader.load_tile_texture(
            &self.device,
            &self.queue,
            self.latitude as f64,
            self.longitude as f64,
            zoom_level,
        ).await {
            self.current_tile_texture = Some(tile_texture.clone());
            
            // Create bind group for the tile texture
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Tile Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(
                            &tile_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            
            self.current_bind_group = Some(bind_group);
        }
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
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.8, g: 0.8, b: 0.9, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            rpass.set_pipeline(&self.pipeline);
            
            // Set bind group if we have tile texture
            if let Some(bind_group) = &self.current_bind_group {
                rpass.set_bind_group(0, bind_group, &[]);
            }
            
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

#[tokio::main]
async fn main() {
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
    app.on_pan_map(move |_dx, _dy| {
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