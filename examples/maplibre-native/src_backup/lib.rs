use cxx::prelude::*;

#[cxx::bridge]
mod ffi {
    unsafe extern "C++" {
        include!("maplibre_bridge.hpp");

        // MapLibre Native Map wrapper
        type MapLibreMap;

        // Map lifecycle
        fn create_map(width: u32, height: u32) -> UniquePtr<MapLibreMap>;
        fn destroy_map(map: UniquePtr<MapLibreMap>);

        // Camera controls
        fn set_camera(map: Pin<&mut MapLibreMap>, latitude: f64, longitude: f64, zoom: f64);
        fn set_bearing(map: Pin<&mut MapLibreMap>, bearing: f64);
        fn set_pitch(map: Pin<&mut MapLibreMap>, pitch: f64);

        // Style management
        fn set_style(map: Pin<&mut MapLibreMap>, style_json: &str) -> bool;

        // Rendering
        fn render_frame(map: Pin<&mut MapLibreMap>) -> bool;
        fn get_texture_id(map: Pin<&mut MapLibreMap>) -> u32;
        fn get_texture_width(map: Pin<&mut MapLibreMap>) -> u32;
        fn get_texture_height(map: Pin<&mut MapLibreMap>) -> u32;

        // Coordinate conversion
        fn screen_to_geographic(
            map: Pin<&mut MapLibreMap>, 
            screen_x: f64, 
            screen_y: f64
        ) -> Vec<f64>; // [lat, lng]
        
        fn geographic_to_screen(
            map: Pin<&mut MapLibreMap>, 
            latitude: f64, 
            longitude: f64
        ) -> Vec<f64>; // [x, y]
    }
}

pub use ffi::*;

// Re-export for convenience
pub type MapLibreMap = ffi::MapLibreMap;