// Simple map shader that creates a tile-like pattern
// Simulates map tiles with different colors based on coordinates

struct MapData {
    viewport: vec4<f32>,  // lat, lng, zoom, time
    pan_offset: vec2<f32>, // pan offset
}

var<push_constant> map_data: MapData;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(-1.0,  3.0),
        vec2<f32>( 3.0, -1.0)
    );
    return vec4<f32>(positions[vertex_index], 0.0, 1.0);
}

@fragment  
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let coord = position.xy;
    let zoom = map_data.viewport.z;
    let time = map_data.viewport.w;
    
    // Add pan offset
    let adjusted_coord = coord + map_data.pan_offset;
    
    // Create tile-like pattern based on zoom level
    let tile_size = 64.0 / zoom;
    let tile_x = floor(adjusted_coord.x / tile_size);
    let tile_y = floor(adjusted_coord.y / tile_size);
    
    // Create checkerboard pattern for tiles
    let checker = (u32(tile_x) + u32(tile_y)) % 2u;
    
    // Base map colors - simulate land/water
    var base_color = vec3<f32>(0.6, 0.8, 0.4); // Land (green)
    if (checker == 1u) {
        base_color = vec3<f32>(0.4, 0.6, 0.9); // Water (blue)
    }
    
    // Add grid lines for tile boundaries
    let grid_x = adjusted_coord.x % tile_size;
    let grid_y = adjusted_coord.y % tile_size;
    let grid_width = 2.0;
    
    if (grid_x < grid_width || grid_y < grid_width) {
        base_color = base_color * 0.7; // Darken grid lines
    }
    
    // Add some variation based on position
    let noise = sin(adjusted_coord.x * 0.01) * sin(adjusted_coord.y * 0.01);
    base_color = base_color + noise * 0.1;
    
    // Add animated effect based on time
    let wave = sin(time + adjusted_coord.x * 0.02 + adjusted_coord.y * 0.02) * 0.05;
    base_color = base_color + wave;
    
    return vec4<f32>(base_color, 1.0);
}