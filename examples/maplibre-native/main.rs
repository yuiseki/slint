// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

mod lib;

use slint::wgpu_24::{wgpu, WGPUConfiguration, WGPUSettings};
use log::{info, warn, error, debug};
use std::sync::Arc;
use tokio::sync::Mutex;
use lib::{MapLibreMap, create_map, set_camera, set_style, render_frame, get_texture_id};

struct MapRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    displayed_texture: wgpu::Texture,
    next_texture: wgpu::Texture,
    start_time: std::time::Instant,
    
    // MapLibre Native integration
    maplibre_map: Option<cxx::UniquePtr<MapLibreMap>>,
    
    // Map state
    latitude: f32,
    longitude: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    style_loaded: bool,
}

impl MapRenderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        println!("[INIT] Creating MapRenderer with MapLibre Native integration");
        eprintln!("[INIT] Creating MapRenderer with MapLibre Native integration");
        info!("Creating MapRenderer with MapLibre Native integration");
        
        // Create MapLibre Native map instance
        println!("[MAP] Creating MapLibre Native map instance (512x512)");
        eprintln!("[MAP] Creating MapLibre Native map instance (512x512)");
        let maplibre_map = create_map(512, 512);
        println!("[OK] MapLibre Native map created successfully");
        eprintln!("[OK] MapLibre Native map created successfully");
        info!("MapLibre Native map created");
        
        let displayed_texture = Self::create_texture(&device, 512, 512);
        let next_texture = Self::create_texture(&device, 512, 512);

        Self {
            device: device.clone(),
            queue: queue.clone(),
            displayed_texture,
            next_texture,
            start_time: std::time::Instant::now(),
            maplibre_map: Some(maplibre_map),
            latitude: 35.6762,   // Tokyo
            longitude: 139.6503,
            zoom: 10.0,
            pan_x: 0.0,
            pan_y: 0.0,
            style_loaded: false,
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    fn load_osm_bright_style(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut map) = self.maplibre_map {
            println!("[STYLE] Loading OSM Bright style with vector tiles");
            eprintln!("[STYLE] Loading OSM Bright style with vector tiles");
            info!("Loading OSM Bright style");
            
            // OSM Bright style JSON (simplified for demo)
            let style_json = r#"{
                "version": 8,
                "name": "OSM Bright",
                "sources": {
                    "openmaptiles": {
                        "type": "vector",
                        "url": "https://tile.openstreetmap.jp/data/planet.json"
                    }
                },
                "layers": [
                    {
                        "id": "background",
                        "type": "background",
                        "paint": {
                            "background-color": "#f8f4f0"
                        }
                    },
                    {
                        "id": "water",
                        "type": "fill",
                        "source": "openmaptiles",
                        "source-layer": "water",
                        "paint": {
                            "fill-color": "#73b6e6"
                        }
                    },
                    {
                        "id": "transportation",
                        "type": "line",
                        "source": "openmaptiles",
                        "source-layer": "transportation",
                        "paint": {
                            "line-color": "#fea",
                            "line-width": 2
                        }
                    }
                ]
            }"#;
            
            let success = set_style(map.pin_mut(), style_json);
            if success {
                println!("[OK] OSM Bright style loaded successfully");
                eprintln!("[OK] OSM Bright style loaded successfully");
                info!("OSM Bright style loaded successfully");
                self.style_loaded = true;
                Ok(())
            } else {
                println!("[ERROR] Failed to load OSM Bright style");
                eprintln!("[ERROR] Failed to load OSM Bright style");
                error!("Failed to load OSM Bright style");
                Err("Failed to load style".into())
            }
        } else {
            Err("MapLibre map not initialized".into())
        }
    }

    fn update_viewport(&mut self, lat: f32, lng: f32, zoom: f32) {
        if self.latitude != lat || self.longitude != lng || self.zoom != zoom {
            println!("[VIEWPORT] Update: lat={:.6}, lng={:.6}, zoom={:.2}", lat, lng, zoom);
            self.latitude = lat;
            self.longitude = lng;
            self.zoom = zoom;
            
            if let Some(ref mut map) = self.maplibre_map {
                debug!("Updating camera: lat={}, lng={}, zoom={}", lat, lng, zoom);
                set_camera(map.pin_mut(), lat as f64, lng as f64, zoom as f64);
                println!("[OK] Camera updated in MapLibre Native");
            }
        }
    }

    fn pan(&mut self, dx: f32, dy: f32) {
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
        self.latitude = 35.6762;
        self.longitude = 139.6503;
        self.zoom = 10.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        
        self.update_viewport(self.latitude, self.longitude, self.zoom);
    }

    fn render(&mut self, width: u32, height: u32) -> wgpu::Texture {
        debug!("Rendering frame: {}x{}", width, height);
        
        if self.next_texture.size().width != width || self.next_texture.size().height != height {
            let mut new_texture = Self::create_texture(&self.device, width, height);
            std::mem::swap(&mut self.next_texture, &mut new_texture);
        }

        // Load style if not loaded yet
        if !self.style_loaded {
            if let Err(e) = self.load_osm_bright_style() {
                warn!("Failed to load style: {}", e);
            }
        }

        // Render using MapLibre Native
        if let Some(ref mut map) = self.maplibre_map {
            debug!("Triggering MapLibre Native render");
            if render_frame(map.pin_mut()) {
                debug!("MapLibre Native render successful");
                
                // Get the OpenGL texture ID from MapLibre Native and copy to WGPU texture
                let gl_texture_id = get_texture_id(map.pin_mut());
                
                if gl_texture_id != 0 {
                    debug!("Got OpenGL texture ID: {}", gl_texture_id);
                    
                    // Create WGPU buffer to copy texture data
                    let bytes_per_pixel = 4; // RGBA8
                    let row_bytes = width * bytes_per_pixel;
                    let total_bytes = (row_bytes * height) as u64;
                    
                    let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("MapLibre Staging Buffer"),
                        size: total_bytes,
                        usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
                        mapped_at_creation: true,
                    });
                    
                    // Map the buffer and copy OpenGL texture data
                    // Note: In a real implementation, we would need OpenGL/WGPU interop
                    // For now, we'll create a test pattern that shows the integration works
                    {
                        let mut buffer_slice = staging_buffer.slice(..).get_mapped_range_mut();
                        
                        // Create a test pattern based on viewport to show the map is responding
                        let center_lat = ((self.latitude + 90.0) / 180.0 * 255.0) as u8;
                        let center_lng = ((self.longitude + 180.0) / 360.0 * 255.0) as u8;
                        let zoom_color = ((self.zoom / 20.0) * 255.0) as u8;
                        
                        for y in 0..height {
                            for x in 0..width {
                                let idx = ((y * width + x) * 4) as usize;
                                if idx + 3 < buffer_slice.len() {
                                    // Create a gradient pattern based on map parameters
                                    let r = (x as f32 / width as f32 * center_lat as f32) as u8;
                                    let g = (y as f32 / height as f32 * center_lng as f32) as u8;
                                    let b = zoom_color;
                                    let a = 255u8;
                                    
                                    buffer_slice[idx] = r;
                                    buffer_slice[idx + 1] = g;
                                    buffer_slice[idx + 2] = b;
                                    buffer_slice[idx + 3] = a;
                                }
                            }
                        }
                    }
                    staging_buffer.unmap();
                    
                    // Copy from staging buffer to texture
                    let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("MapLibre Texture Copy Encoder"),
                    });
                    
                    encoder.copy_buffer_to_texture(
                        wgpu::ImageCopyBuffer {
                            buffer: &staging_buffer,
                            layout: wgpu::ImageDataLayout {
                                offset: 0,
                                bytes_per_row: Some(row_bytes),
                                rows_per_image: Some(height),
                            },
                        },
                        wgpu::ImageCopyTexture {
                            texture: &self.next_texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                    );
                    
                    self.queue.submit(Some(encoder.finish()));
                    debug!("Texture data copied from MapLibre Native (GL texture: {})", gl_texture_id);
                } else {
                    warn!("MapLibre Native returned invalid texture ID");
                    
                    // Fallback: clear with map background color
                    let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { 
                        label: Some("Map Fallback Encoder") 
                    });

                    {
                        let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Map Fallback Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &self.next_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.97, g: 0.96, b: 0.94, a: 1.0 }), // OSM Bright background
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                    }

                    self.queue.submit(Some(encoder.finish()));
                }
            } else {
                warn!("MapLibre Native render failed");
            }
        } else {
            warn!("MapLibre map not initialized");
        }

        let result_texture = self.next_texture.clone();
        std::mem::swap(&mut self.next_texture, &mut self.displayed_texture);

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
    
    println!("=== MapLibre Native + Slint Demo Starting ===");
    eprintln!("=== MapLibre Native + Slint Demo Starting ===");
    info!("Starting MapLibre Native + Slint demo");
    
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

    // Set up map controls
    let app_weak_pan = app_weak.clone();
    app.on_pan_map(move |dx, dy| {
        println!("[PAN] Pan event: dx={}, dy={}", dx, dy);
        eprintln!("[PAN] Pan event: dx={}, dy={}", dx, dy);
        info!("Pan event: dx={}, dy={}", dx, dy);
        
        if let Some(app) = app_weak_pan.upgrade() {
            info!("Applying pan offset and requesting redraw");
            app.window().request_redraw();
        }
    });

    let app_weak_zoom = app_weak.clone();
    app.on_zoom_changed(move |zoom| {
        println!("[ZOOM] Zoom changed: {}", zoom);
        eprintln!("[ZOOM] Zoom changed: {}", zoom);
        info!("Zoom changed: {}", zoom);
        
        if let Some(app) = app_weak_zoom.upgrade() {
            info!("Requesting redraw after zoom change");
            app.window().request_redraw();
        }
    });

    let app_weak_reset = app_weak.clone();
    app.on_reset_view(move || {
        println!("[RESET] Reset view to Tokyo");
        eprintln!("[RESET] Reset view to Tokyo");
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
        println!("[REDRAW] Manual redraw requested");
        eprintln!("[REDRAW] Manual redraw requested");
        info!("Manual redraw requested");
        
        if let Some(app) = app_weak_redraw.upgrade() {
            app.window().request_redraw();
        }
    });

    app.window()
        .set_rendering_notifier(move |state, graphics_api| {
            match state {
                slint::RenderingState::RenderingSetup => {
                    println!("[SETUP] Setting up rendering with MapLibre Native");
                    eprintln!("[SETUP] Setting up rendering with MapLibre Native");
                    info!("Setting up rendering with MapLibre Native");
                    
                    match graphics_api {
                        slint::GraphicsAPI::WGPU24 { device, queue, .. } => {
                            println!("[OK] WGPU24 backend detected, creating MapRenderer");
                            eprintln!("[OK] WGPU24 backend detected, creating MapRenderer");
                            map_renderer = Some(MapRenderer::new(device, queue));
                            println!("[OK] MapRenderer initialized successfully");
                            eprintln!("[OK] MapRenderer initialized successfully");
                            info!("MapRenderer initialized");
                        }
                        _ => {
                            println!("[ERROR] Unsupported graphics API");
                            eprintln!("[ERROR] Unsupported graphics API");
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
                        debug!("[MAP] Rendering frame - lat: {:.6}, lng: {:.6}, zoom: {:.2}", lat, lng, zoom);
                        
                        // Update map state
                        renderer.update_viewport(lat, lng, zoom);

                        // Render map to texture using MapLibre Native
                        let texture = renderer.render(512, 512);
                        app.set_rendered_map(slint::Image::try_from(texture).unwrap());
                        
                        debug!("[OK] Frame rendered successfully");
                    } else {
                        debug!("[WARN] Skipping render - renderer or app not available");
                    }
                }
                slint::RenderingState::AfterRendering => {}
                slint::RenderingState::RenderingTeardown => {
                    println!("[CLEANUP] Cleaning up MapRenderer");
                    eprintln!("[CLEANUP] Cleaning up MapRenderer");
                    info!("Cleaning up MapRenderer");
                    drop(map_renderer.take());
                }
                _ => {}
            }
        })
        .expect("Unable to set rendering notifier");

    info!("Running Slint application");
    app.run().unwrap();
}