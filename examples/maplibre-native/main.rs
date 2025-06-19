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
        info!("Creating MapRenderer with MapLibre Native integration");
        
        // Create MapLibre Native map instance
        let maplibre_map = create_map(512, 512);
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
                info!("OSM Bright style loaded successfully");
                self.style_loaded = true;
                Ok(())
            } else {
                error!("Failed to load OSM Bright style");
                Err("Failed to load style".into())
            }
        } else {
            Err("MapLibre map not initialized".into())
        }
    }

    fn update_viewport(&mut self, lat: f32, lng: f32, zoom: f32) {
        if self.latitude != lat || self.longitude != lng || self.zoom != zoom {
            self.latitude = lat;
            self.longitude = lng;
            self.zoom = zoom;
            
            if let Some(ref mut map) = self.maplibre_map {
                debug!("Updating camera: lat={}, lng={}, zoom={}", lat, lng, zoom);
                set_camera(map.pin_mut(), lat as f64, lng as f64, zoom as f64);
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
                
                // TODO: Get the OpenGL texture from MapLibre Native and copy to WGPU texture
                // For now, we'll create a placeholder
                let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { 
                    label: Some("Map Render Encoder") 
                });

                // Clear with a map-like color
                {
                    let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Map Clear Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &self.next_texture.create_view(&wgpu::TextureViewDescriptor::default()),
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.8, g: 0.9, b: 0.8, a: 1.0 }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                }

                self.queue.submit(Some(encoder.finish()));
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
    // Initialize logger
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();
    
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
        if let Some(app) = app_weak_pan.upgrade() {
            // TODO: Apply pan offset to MapLibre Native
            info!("Pan: dx={}, dy={}", dx, dy);
            app.window().request_redraw();
        }
    });

    let app_weak_zoom = app_weak.clone();
    app.on_zoom_changed(move |zoom| {
        if let Some(app) = app_weak_zoom.upgrade() {
            info!("Zoom changed: {}", zoom);
            app.window().request_redraw();
        }
    });

    let app_weak_reset = app_weak.clone();
    app.on_reset_view(move || {
        if let Some(app) = app_weak_reset.upgrade() {
            info!("Reset view");
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
                    info!("Setting up rendering with MapLibre Native");
                    match graphics_api {
                        slint::GraphicsAPI::WGPU24 { device, queue, .. } => {
                            map_renderer = Some(MapRenderer::new(device, queue));
                            info!("MapRenderer initialized");
                        }
                        _ => {
                            error!("Unsupported graphics API");
                            return;
                        }
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

                        // Render map to texture using MapLibre Native
                        let texture = renderer.render(512, 512);
                        app.set_rendered_map(slint::Image::try_from(texture).unwrap());
                    }
                }
                slint::RenderingState::AfterRendering => {}
                slint::RenderingState::RenderingTeardown => {
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