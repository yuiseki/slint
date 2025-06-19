// Vector tile map shader that renders actual tile images
// Supports texture sampling for real map data

struct MapData {
    viewport: vec4<f32>,  // lat, lng, zoom, time
    pan_offset: vec2<f32>, // pan offset
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

var<push_constant> map_data: MapData;

@group(0) @binding(0)
var tile_texture: texture_2d<f32>;
@group(0) @binding(1)
var tile_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(-1.0,  3.0),
        vec2<f32>( 3.0, -1.0)
    );
    
    let tex_coords = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, -1.0),
        vec2<f32>(2.0, 1.0)
    );
    
    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.tex_coord = tex_coords[vertex_index];
    return output;
}

@fragment  
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let coord = input.position.xy;
    let zoom = map_data.viewport.z;
    let lat = map_data.viewport.x;
    let lng = map_data.viewport.y;
    
    // Calculate tile coordinates based on position and zoom
    let tile_size = 256.0;
    let adjusted_coord = coord + map_data.pan_offset * zoom;
    
    // Map screen coordinates to texture coordinates
    let screen_center = vec2<f32>(256.0, 256.0); // Assuming 512x512 texture
    let offset_coord = adjusted_coord - screen_center;
    
    // Calculate texture coordinates with proper scaling
    let scale = pow(2.0, zoom - 10.0); // Scale based on zoom level
    let tex_x = (offset_coord.x / tile_size) * scale + 0.5;
    let tex_y = (offset_coord.y / tile_size) * scale + 0.5;
    
    let tex_coord = vec2<f32>(tex_x, tex_y);
    
    // Sample the tile texture
    if (tex_coord.x >= 0.0 && tex_coord.x <= 1.0 && 
        tex_coord.y >= 0.0 && tex_coord.y <= 1.0) {
        return textureSample(tile_texture, tile_sampler, tex_coord);
    } else {
        // Fallback color for areas outside tile bounds
        return vec4<f32>(0.8, 0.8, 0.9, 1.0); // Light blue background
    }
}