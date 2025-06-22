// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

use slint::wgpu_24::{wgpu, WGPUConfiguration, WGPUSettings};
use log::{info, warn, error, debug};

struct SimpleMapRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    displayed_texture: wgpu::Texture,
    next_texture: wgpu::Texture,
    
    // Map state
    latitude: f32,
    longitude: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
}

impl SimpleMapRenderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        println!("🏗️  Creating SimpleMapRenderer (without MapLibre Native)");
        eprintln!("🏗️  Creating SimpleMapRenderer (without MapLibre Native)");
        info!("Creating SimpleMapRenderer");
        
        let displayed_texture = Self::create_texture(&device, 512, 512);
        let next_texture = Self::create_texture(&device, 512, 512);

        Self {
            device: device.clone(),
            queue: queue.clone(),
            displayed_texture,
            next_texture,
            latitude: 35.6762,   // Tokyo
            longitude: 139.6503,
            zoom: 10.0,
            pan_x: 0.0,
            pan_y: 0.0,
        }
    }

    fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Simple Map Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    fn update_viewport(&mut self, lat: f32, lng: f32, zoom: f32) {
        if self.latitude != lat || self.longitude != lng || self.zoom != zoom {
            println!("📍 Viewport update: lat={:.6}, lng={:.6}, zoom={:.2}", lat, lng, zoom);
            eprintln!("📍 Viewport update: lat={:.6}, lng={:.6}, zoom={:.2}", lat, lng, zoom);
            self.latitude = lat;
            self.longitude = lng;
            self.zoom = zoom;
            println!("✅ Viewport updated in simple renderer");
        }
    }

    fn pan(&mut self, dx: f32, dy: f32) {
        println!("🖱️  Pan operation: dx={}, dy={}", dx, dy);
        eprintln!("🖱️  Pan operation: dx={}, dy={}", dx, dy);
        
        let scale = 1.0 / self.zoom;
        self.pan_x += dx * scale;
        self.pan_y += dy * scale;
        
        // Convert pan to lat/lng offset
        let lat_offset = dy * scale * 0.001;
        let lng_offset = dx * scale * 0.001;
        
        self.update_viewport(
            self.latitude + lat_offset, 
            self.longitude + lng_offset, 
            self.zoom
        );
    }

    fn reset_view(&mut self) {
        println!("🔄 Resetting view to Tokyo");
        eprintln!("🔄 Resetting view to Tokyo");
        
        self.latitude = 35.6762;
        self.longitude = 139.6503;
        self.zoom = 10.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        
        self.update_viewport(self.latitude, self.longitude, self.zoom);
    }

    fn render(&mut self, width: u32, height: u32) -> wgpu::Texture {
        debug!("🎨 Rendering simple frame: {}x{}", width, height);
        
        if self.next_texture.size().width != width || self.next_texture.size().height != height {
            let mut new_texture = Self::create_texture(&self.device, width, height);
            std::mem::swap(&mut self.next_texture, &mut new_texture);
        }

        // Create a simple gradient based on map position and zoom
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { 
            label: Some("Simple Map Render Encoder") 
        });

        // Create a color based on zoom and position for visual feedback
        let zoom_factor = (self.zoom - 1.0) / 19.0; // Normalize zoom to 0-1
        let lat_factor = (self.latitude + 90.0) / 180.0; // Normalize latitude to 0-1
        let lng_factor = (self.longitude + 180.0) / 360.0; // Normalize longitude to 0-1
        
        let clear_color = wgpu::Color { 
            r: lng_factor as f64, 
            g: lat_factor as f64, 
            b: zoom_factor as f64, 
            a: 1.0 
        };

        {
            let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Simple Map Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.next_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        self.queue.submit(Some(encoder.finish()));

        let result_texture = self.next_texture.clone();
        std::mem::swap(&mut self.next_texture, &mut self.displayed_texture);

        debug!("✅ Simple frame rendered successfully");
        result_texture
    }
}

#[tokio::main]
async fn main() {
    // Initialize logger with explicit configuration for Slint Live-preview
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        .format_timestamp_millis()
        .init();
    
    println!("=== Simple MapLibre + Slint Demo Starting ===");
    eprintln!("=== Simple MapLibre + Slint Demo Starting ===");
    info!("Starting Simple MapLibre + Slint demo");
    
    let mut wgpu_settings = WGPUSettings::default();
    wgpu_settings.device_required_features = wgpu::Features::empty();
    wgpu_settings.device_required_limits = wgpu::Limits::default();

    slint::BackendSelector::new()
        .require_wgpu_24(WGPUConfiguration::Automatic(wgpu_settings))
        .select()
        .expect("Unable to create Slint backend with WGPU renderer");

    let app = MapLibreDemo::new().unwrap();
    let mut map_renderer = None;
    let app_weak = app.as_weak();

    // Set up map controls with detailed logging
    let app_weak_pan = app_weak.clone();
    app.on_pan_map(move |dx, dy| {
        println!("🖱️  Pan event: dx={}, dy={}", dx, dy);
        eprintln!("🖱️  Pan event: dx={}, dy={}", dx, dy);
        info!("Pan event: dx={}, dy={}", dx, dy);
        
        if let Some(app) = app_weak_pan.upgrade() {
            info!("Requesting redraw after pan");
            app.window().request_redraw();
        }
    });

    let app_weak_zoom = app_weak.clone();
    app.on_zoom_changed(move |zoom| {
        println!("🔍 Zoom changed: {}", zoom);
        eprintln!("🔍 Zoom changed: {}", zoom);
        info!("Zoom changed: {}", zoom);
        
        if let Some(app) = app_weak_zoom.upgrade() {
            info!("Requesting redraw after zoom change");
            app.window().request_redraw();
        }
    });

    let app_weak_reset = app_weak.clone();
    app.on_reset_view(move || {
        println!("🏠 Reset view to Tokyo");
        eprintln!("🏠 Reset view to Tokyo");
        info!("Reset view to Tokyo");
        
        if let Some(app) = app_weak_reset.upgrade() {
            app.set_latitude(35.6762);
            app.set_longitude(139.6503);
            app.set_zoom_level(10.0);
            info!("View reset complete, requesting redraw");
            app.window().request_redraw();
        }
    });

    let app_weak_redraw = app_weak.clone();
    app.on_request_redraw(move || {
        println!("🎨 Manual redraw requested");
        eprintln!("🎨 Manual redraw requested");
        info!("Manual redraw requested");
        
        if let Some(app) = app_weak_redraw.upgrade() {
            app.window().request_redraw();
        }
    });

    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            match state {
                slint::RenderingState::RenderingSetup => {
                    println!("🚀 Setting up simple rendering");
                    eprintln!("🚀 Setting up simple rendering");
                    info!("Setting up simple rendering");
                    
                    match graphics_api {
                        slint::GraphicsAPI::WGPU24 { device, queue, .. } => {
                            println!("✅ WGPU24 backend detected, creating SimpleMapRenderer");
                            eprintln!("✅ WGPU24 backend detected, creating SimpleMapRenderer");
                            map_renderer = Some(SimpleMapRenderer::new(device, queue));
                            println!("✅ SimpleMapRenderer initialized successfully");
                            eprintln!("✅ SimpleMapRenderer initialized successfully");
                            info!("SimpleMapRenderer initialized");
                        }
                        _ => {
                            println!("❌ Unsupported graphics API");
                            eprintln!("❌ Unsupported graphics API");
                            error!("Unsupported graphics API");
                            return;
                        }
                    };
                }
                slint::RenderingState::BeforeRendering => {
                    if let (Some(renderer), Some(app)) = (map_renderer.as_mut(), app_weak.upgrade()) {
                        let lat = app.get_latitude();
                        let lng = app.get_longitude();
                        let zoom = app.get_zoom_level();
                        
                        // Debug current map state
                        debug!("🗺️  Rendering frame - lat: {:.6}, lng: {:.6}, zoom: {:.2}", lat, lng, zoom);
                        
                        // Update map state
                        renderer.update_viewport(lat, lng, zoom);

                        // Render simple map
                        let texture = renderer.render(512, 512);
                        app.set_rendered_map(slint::Image::try_from(texture).unwrap());
                        
                        debug!("✅ Frame rendered successfully");
                    } else {
                        debug!("⚠️  Skipping render - renderer or app not available");
                    }
                }
                slint::RenderingState::AfterRendering => {}
                slint::RenderingState::RenderingTeardown => {
                    println!("🧹 Cleaning up SimpleMapRenderer");
                    eprintln!("🧹 Cleaning up SimpleMapRenderer");
                    info!("Cleaning up SimpleMapRenderer");
                    drop(map_renderer.take());
                }
                _ => {}
            }
        })
        .expect("Unable to set rendering notifier");

    println!("🎮 Running Slint application with detailed logging");
    eprintln!("🎮 Running Slint application with detailed logging");
    info!("Running Slint application");
    app.run().unwrap();
}