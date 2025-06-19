<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# MapLibre Native Example

This example demonstrates how to integrate map rendering with Slint using WGPU:

1. A map pattern is rendered using WGPU shaders into a texture.
2. The texture is imported into a `slint::Image` and displayed in the UI.
3. Interactive controls allow panning and zooming the map view.

This is implemented using the `set_rendering_notifier` function on the `slint::Window` type. The `BeforeRendering` phase renders the map pattern with WGPU into a texture, which is then imported and displayed by Slint.

The map pattern is procedurally generated using a WGSL shader that creates a tile-like appearance with zoom-responsive scaling and pan offset support.