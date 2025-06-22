#include "maplibre_bridge.hpp"

// MapLibre Native includes
#include <mbgl/map/map.hpp>
#include <mbgl/map/map_options.hpp>
#include <mbgl/storage/resource_options.hpp>
#include <mbgl/util/client_options.hpp>
#include <mbgl/map/camera.hpp>
#include <mbgl/style/style.hpp>
#include <mbgl/util/geometry.hpp>
#include <mbgl/util/geo.hpp>
#include <mbgl/renderer/renderer_frontend.hpp>
#include <mbgl/gfx/renderer_backend.hpp>
#include <mbgl/gl/renderer_backend.hpp>
#include <mbgl/util/run_loop.hpp>
#include <mbgl/storage/file_source_manager.hpp>
#include <mbgl/storage/default_file_source.hpp>

#include <GL/gl.h>
#include <iostream>
#include <thread>

// Simple OpenGL backend for offscreen rendering
class OffscreenRendererBackend : public mbgl::gl::RendererBackend {
public:
    OffscreenRendererBackend(uint32_t width, uint32_t height) 
        : width_(width), height_(height), texture_id_(0) {
        
        // Create framebuffer and texture
        glGenFramebuffers(1, &framebuffer_);
        glGenTextures(1, &texture_id_);
        
        glBindTexture(GL_TEXTURE_2D, texture_id_);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, width, height, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        
        glBindFramebuffer(GL_FRAMEBUFFER, framebuffer_);
        glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, texture_id_, 0);
        
        if (glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE) {
            std::cerr << "Framebuffer not complete!" << std::endl;
        }
        
        glBindFramebuffer(GL_FRAMEBUFFER, 0);
    }
    
    ~OffscreenRendererBackend() {
        if (texture_id_) glDeleteTextures(1, &texture_id_);
        if (framebuffer_) glDeleteFramebuffers(1, &framebuffer_);
    }
    
    uint32_t getTextureId() const { return texture_id_; }
    uint32_t getWidth() const { return width_; }
    uint32_t getHeight() const { return height_; }

protected:
    mbgl::gfx::ContextMode getContextMode() override {
        return mbgl::gfx::ContextMode::Unique;
    }
    
    void activate() override {
        glBindFramebuffer(GL_FRAMEBUFFER, framebuffer_);
        glViewport(0, 0, width_, height_);
    }
    
    void deactivate() override {
        glBindFramebuffer(GL_FRAMEBUFFER, 0);
    }

private:
    uint32_t width_, height_;
    GLuint framebuffer_ = 0;
    GLuint texture_id_ = 0;
};

// Simple renderer frontend
class SimpleRendererFrontend : public mbgl::RendererFrontend {
public:
    SimpleRendererFrontend(std::unique_ptr<mbgl::gfx::RendererBackend> backend)
        : backend_(std::move(backend)) {}
    
    void reset() override {}
    void setObserver(mbgl::RendererObserver&) override {}
    
    void update(std::shared_ptr<mbgl::UpdateParameters>) override {}
    
    void render(const mbgl::CameraOptions&) override {
        if (backend_) {
            backend_->activate();
            // Rendering happens through the Map's render calls
            backend_->deactivate();
        }
    }
    
    mbgl::gfx::RendererBackend* getBackend() override {
        return backend_.get();
    }

private:
    std::unique_ptr<mbgl::gfx::RendererBackend> backend_;
};

// Simple map observer
class SimpleMapObserver : public mbgl::MapObserver {
public:
    void onCameraWillChange(mbgl::MapObserver::CameraChangeMode) override {}
    void onCameraIsChanging() override {}
    void onCameraDidChange(mbgl::MapObserver::CameraChangeMode) override {}
    void onWillStartLoadingMap() override {}
    void onDidFinishLoadingMap() override {}
    void onDidFailLoadingMap(mbgl::MapLoadError, const std::string&) override {}
    void onWillStartRenderingFrame() override {}
    void onDidFinishRenderingFrame(mbgl::MapObserver::RenderFrameStatus) override {}
    void onWillStartRenderingMap() override {}
    void onDidFinishRenderingMap(mbgl::MapObserver::RenderMode) override {}
    void onDidFinishLoadingStyle() override {}
    void onSourceChanged(mbgl::style::Source&) override {}
    void onDidBecomeIdle() override {}
    void onStyleImageMissing(const std::string&, const std::function<void()>&) override {}
    void onCanRemoveUnusedStyleImage(const std::string&) override { return true; }
};

// Implementation class
class MapLibreMap::Impl {
public:
    Impl(uint32_t width, uint32_t height) 
        : width_(width), height_(height) {
        
        // Initialize RunLoop for this thread if needed
        if (!mbgl::util::RunLoop::Get()) {
            run_loop_ = std::make_unique<mbgl::util::RunLoop>();
        }
        
        // Create backend and frontend
        auto backend = std::make_unique<OffscreenRendererBackend>(width, height);
        backend_ptr_ = backend.get();
        frontend_ = std::make_unique<SimpleRendererFrontend>(std::move(backend));
        
        // Create observer
        observer_ = std::make_unique<SimpleMapObserver>();
        
        // Setup resource options
        resource_options_ = mbgl::ResourceOptions()
            .withCachePath("./cache")
            .withAssetPath("./assets");
            
        // Setup map options
        map_options_ = mbgl::MapOptions()
            .withMode(mbgl::MapMode::Static)
            .withSize(mbgl::Size{width, height});
            
        // Create client options
        client_options_ = mbgl::ClientOptions();
        
        // Create the map
        map_ = std::make_unique<mbgl::Map>(
            *frontend_,
            *observer_,
            map_options_,
            resource_options_,
            client_options_
        );
    }
    
    ~Impl() = default;
    
    void set_camera(double latitude, double longitude, double zoom) {
        if (map_) {
            mbgl::CameraOptions camera;
            camera.center = mbgl::LatLng{latitude, longitude};
            camera.zoom = zoom;
            map_->jumpTo(camera);
        }
    }
    
    void set_bearing(double bearing) {
        if (map_) {
            mbgl::CameraOptions camera;
            camera.bearing = bearing;
            map_->jumpTo(camera);
        }
    }
    
    void set_pitch(double pitch) {
        if (map_) {
            mbgl::CameraOptions camera;
            camera.pitch = pitch;
            map_->jumpTo(camera);
        }
    }
    
    bool set_style(const std::string& style_json) {
        if (map_) {
            try {
                map_->getStyle().loadJSON(style_json);
                return true;
            } catch (const std::exception& e) {
                std::cerr << "Failed to load style: " << e.what() << std::endl;
                return false;
            }
        }
        return false;
    }
    
    bool render_frame() {
        if (map_) {
            try {
                map_->renderStill([](std::exception_ptr) {
                    // Render callback - could handle errors here
                });
                return true;
            } catch (const std::exception& e) {
                std::cerr << "Failed to render frame: " << e.what() << std::endl;
                return false;
            }
        }
        return false;
    }
    
    uint32_t get_texture_id() const {
        return backend_ptr_ ? backend_ptr_->getTextureId() : 0;
    }
    
    uint32_t get_texture_width() const {
        return width_;
    }
    
    uint32_t get_texture_height() const {
        return height_;
    }
    
    std::vector<double> screen_to_geographic(double screen_x, double screen_y) {
        if (map_) {
            auto latLng = map_->latLngForPixel(mbgl::ScreenCoordinate{screen_x, screen_y});
            return {latLng.latitude(), latLng.longitude()};
        }
        return {0.0, 0.0};
    }
    
    std::vector<double> geographic_to_screen(double latitude, double longitude) {
        if (map_) {
            auto screenCoord = map_->pixelForLatLng(mbgl::LatLng{latitude, longitude});
            return {screenCoord.x, screenCoord.y};
        }
        return {0.0, 0.0};
    }

private:
    uint32_t width_, height_;
    std::unique_ptr<mbgl::util::RunLoop> run_loop_;
    std::unique_ptr<mbgl::Map> map_;
    std::unique_ptr<SimpleRendererFrontend> frontend_;
    std::unique_ptr<SimpleMapObserver> observer_;
    OffscreenRendererBackend* backend_ptr_;
    mbgl::ResourceOptions resource_options_;
    mbgl::MapOptions map_options_;
    mbgl::ClientOptions client_options_;
};

// MapLibreMap implementation
MapLibreMap::MapLibreMap(uint32_t width, uint32_t height) 
    : pImpl(std::make_unique<Impl>(width, height)) {}

MapLibreMap::~MapLibreMap() = default;

void MapLibreMap::set_camera(double latitude, double longitude, double zoom) {
    pImpl->set_camera(latitude, longitude, zoom);
}

void MapLibreMap::set_bearing(double bearing) {
    pImpl->set_bearing(bearing);
}

void MapLibreMap::set_pitch(double pitch) {
    pImpl->set_pitch(pitch);
}

bool MapLibreMap::set_style(const std::string& style_json) {
    return pImpl->set_style(style_json);
}

bool MapLibreMap::render_frame() {
    return pImpl->render_frame();
}

uint32_t MapLibreMap::get_texture_id() const {
    return pImpl->get_texture_id();
}

uint32_t MapLibreMap::get_texture_width() const {
    return pImpl->get_texture_width();
}

uint32_t MapLibreMap::get_texture_height() const {
    return pImpl->get_texture_height();
}

std::vector<double> MapLibreMap::screen_to_geographic(double screen_x, double screen_y) {
    return pImpl->screen_to_geographic(screen_x, screen_y);
}

std::vector<double> MapLibreMap::geographic_to_screen(double latitude, double longitude) {
    return pImpl->geographic_to_screen(latitude, longitude);
}

// C interface functions
std::unique_ptr<MapLibreMap> create_map(uint32_t width, uint32_t height) {
    return std::make_unique<MapLibreMap>(width, height);
}

void destroy_map(std::unique_ptr<MapLibreMap> map) {
    // Automatic cleanup through unique_ptr destructor
}

void set_camera(MapLibreMap& map, double latitude, double longitude, double zoom) {
    map.set_camera(latitude, longitude, zoom);
}

void set_bearing(MapLibreMap& map, double bearing) {
    map.set_bearing(bearing);
}

void set_pitch(MapLibreMap& map, double pitch) {
    map.set_pitch(pitch);
}

bool set_style(MapLibreMap& map, const std::string& style_json) {
    return map.set_style(style_json);
}

bool render_frame(MapLibreMap& map) {
    return map.render_frame();
}

uint32_t get_texture_id(MapLibreMap& map) {
    return map.get_texture_id();
}

uint32_t get_texture_width(MapLibreMap& map) {
    return map.get_texture_width();
}

uint32_t get_texture_height(MapLibreMap& map) {
    return map.get_texture_height();
}

std::vector<double> screen_to_geographic(MapLibreMap& map, double screen_x, double screen_y) {
    return map.screen_to_geographic(screen_x, screen_y);
}

std::vector<double> geographic_to_screen(MapLibreMap& map, double latitude, double longitude) {
    return map.geographic_to_screen(latitude, longitude);
}