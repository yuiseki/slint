#pragma once

#include <memory>
#include <string>
#include <vector>
#include "rust/cxx.h"

// Forward declarations for MapLibre Native
namespace mbgl {
    class Map;
    class RendererFrontend;
    class RendererBackend;
    class MapObserver;
    class ThreadPool;
    class FileSource;
    class ResourceOptions;
    class MapOptions;
    class ClientOptions;
}

class MapLibreMap {
public:
    MapLibreMap(uint32_t width, uint32_t height);
    ~MapLibreMap();

    // Camera controls
    void set_camera(double latitude, double longitude, double zoom);
    void set_bearing(double bearing);
    void set_pitch(double pitch);

    // Style management
    bool set_style(const std::string& style_json);

    // Rendering
    bool render_frame();
    uint32_t get_texture_id() const;
    uint32_t get_texture_width() const;
    uint32_t get_texture_height() const;

    // Coordinate conversion
    std::vector<double> screen_to_geographic(double screen_x, double screen_y);
    std::vector<double> geographic_to_screen(double latitude, double longitude);

private:
    class Impl;
    std::unique_ptr<Impl> pImpl;
};

// C interface functions for CXX
std::unique_ptr<MapLibreMap> create_map(uint32_t width, uint32_t height);
void destroy_map(std::unique_ptr<MapLibreMap> map);

void set_camera(MapLibreMap& map, double latitude, double longitude, double zoom);
void set_bearing(MapLibreMap& map, double bearing);
void set_pitch(MapLibreMap& map, double pitch);

bool set_style(MapLibreMap& map, const std::string& style_json);

bool render_frame(MapLibreMap& map);
uint32_t get_texture_id(MapLibreMap& map);
uint32_t get_texture_width(MapLibreMap& map);
uint32_t get_texture_height(MapLibreMap& map);

std::vector<double> screen_to_geographic(MapLibreMap& map, double screen_x, double screen_y);
std::vector<double> geographic_to_screen(MapLibreMap& map, double latitude, double longitude);