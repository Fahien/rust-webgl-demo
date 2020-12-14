/// @todo Draw 3 windows
/// @todo Make the GUI mode immediate
/// @todo Make window content scrollable
/// @todo Make window resizable
/// @todo Draw the scene using the select pipeline onto a texture
/// @todo Show that texture within a window
use super::*;

use nalgebra::Matrix4;
use std::ops::Deref;
use std::{convert::From, ops::DerefMut};

struct GuiPipeline {
    program: Program,
    position_loc: i32,
    uv_loc: i32,
    color_loc: Option<WebGlUniformLocation>,
    transform_loc: Option<WebGlUniformLocation>,
    view_loc: Option<WebGlUniformLocation>,
    proj_loc: Option<WebGlUniformLocation>,
    sampler_loc: Option<WebGlUniformLocation>,
}

impl GuiPipeline {
    fn new(gl: &GL) -> Self {
        let vert_src = include_str!("../res/shader/gui.vert.glsl");
        let frag_src = include_str!("../res/shader/gui.frag.glsl");
        let program = Program::new(gl.clone(), vert_src, frag_src);
        program.bind();

        let position_loc = program.get_attrib_loc("in_position");
        let uv_loc = program.get_attrib_loc("in_uv");
        let color_loc = program.get_uniform_loc("color");
        let transform_loc = program.get_uniform_loc("transform");
        let view_loc = program.get_uniform_loc("view");
        let proj_loc = program.get_uniform_loc("proj");
        let sampler_loc = program.get_uniform_loc("tex_sampler");

        Self {
            program,
            position_loc,
            uv_loc,
            color_loc,
            transform_loc,
            view_loc,
            proj_loc,
            sampler_loc,
        }
    }

    fn draw(&self, primitive: &Primitive) {
        primitive.bind();
        self.bind_attribs();
        primitive.draw();
    }

    fn draw_char(&self, primitive: &Primitive, c: char) {
        primitive.bind();
        self.bind_char_attribs(c);
        primitive.draw();
    }

    fn bind_attribs(&self) {
        // Position
        // Number of bytes between each vertex element
        let stride = std::mem::size_of::<Vertex>() as i32;
        // Offset of vertex data from the beginning of the buffer
        let offset = 0;

        self.program.gl.vertex_attrib_pointer_with_i32(
            self.position_loc as u32,
            3,
            GL::FLOAT,
            false,
            stride,
            offset,
        );
        self.program
            .gl
            .enable_vertex_attrib_array(self.position_loc as u32);

        // Texture coordinates
        let offset = 10 * std::mem::size_of::<f32>() as i32;
        self.program.gl.vertex_attrib_pointer_with_i32(
            self.uv_loc as u32,
            2,
            GL::FLOAT,
            false,
            stride,
            offset,
        );
        self.program
            .gl
            .enable_vertex_attrib_array(self.uv_loc as u32)
    }

    fn bind_char_attribs(&self, c: char) {
        let index = (c as u8 + 53 as u8) as i32;

        // Position
        // Number of bytes between each vertex element
        let stride = std::mem::size_of::<FontVertex>() as i32;
        // Offset of vertex data from the beginning of the buffer
        let offset = 0;

        self.program.gl.vertex_attrib_pointer_with_i32(
            self.position_loc as u32,
            3,
            GL::FLOAT,
            false,
            stride,
            offset,
        );
        self.program
            .gl
            .enable_vertex_attrib_array(self.position_loc as u32);

        // Texture coordinates
        let stride = std::mem::size_of::<UV>() as i32;
        let offset = 4 * std::mem::size_of::<FontVertex>() as i32 + index * stride * 4;
        self.program.gl.vertex_attrib_pointer_with_i32(
            self.uv_loc as u32,
            2,
            GL::FLOAT,
            false,
            stride,
            offset,
        );
        self.program
            .gl
            .enable_vertex_attrib_array(self.uv_loc as u32);
    }

    fn set_color(&self, color: &[f32; 4]) {
        self.program
            .gl
            .uniform4fv_with_f32_array(self.color_loc.as_ref(), color)
    }

    fn set_transform(&self, transform: &Matrix4<f32>) {
        self.program.gl.uniform_matrix4fv_with_f32_array(
            self.transform_loc.as_ref(),
            false,
            transform.as_slice(),
        )
    }

    fn set_view(&self, view: &Matrix4<f32>) {
        self.program.gl.uniform_matrix4fv_with_f32_array(
            self.view_loc.as_ref(),
            false,
            view.as_slice(),
        )
    }

    fn set_proj(&self, proj: &Matrix4<f32>) {
        self.program.gl.uniform_matrix4fv_with_f32_array(
            self.proj_loc.as_ref(),
            false,
            proj.as_slice(),
        )
    }

    fn set_sampler(&self, texture_unit: i32) {
        self.program
            .gl
            .uniform1i(self.sampler_loc.as_ref(), texture_unit);
    }
}

pub struct Gui {
    width: u32,
    height: u32,

    pipeline: GuiPipeline,

    view: Matrix4<f32>,
    proj: Matrix4<f32>,

    /// Generic unit quad with default UVs
    quad: Primitive,
    /// Generic window background
    background: Primitive,
    /// Generic window title bar
    title_bar: Primitive,
    /// Primitive for the shadow
    shadow: Primitive,

    texture: Texture,

    pub windows: Vec<Window>,

    // List of indices into the windows list ordered by farther to closer
    windows_order: Vec<usize>,

    font: Font,

    // Global window title height
    title_height: u32,

    // Index of the window which is focused
    pub focus: Option<usize>,

    // Whether we are draggin the foreground window or not
    dragging: bool,
}

impl Gui {
    fn create_quad(gl: GL) -> Primitive {
        let mut quad = Geometry::<Vertex>::quad();

        quad.vertices[0].uv = [0.0, 0.0];
        quad.vertices[1].uv = [1.0, 0.0];
        quad.vertices[2].uv = [1.0, 1.0];
        quad.vertices[3].uv = [0.0, 1.0];

        Primitive::new(gl, &quad)
    }


    fn create_background(gl: GL) -> Primitive {
        let mut quad = Geometry::<Vertex>::quad();

        quad.vertices[0].uv = [0.0, 1.0 / 4.0];
        quad.vertices[1].uv = [1.0, 1.0 / 4.0];
        quad.vertices[2].uv = [1.0, 2.0 / 4.0];
        quad.vertices[3].uv = [0.0, 2.0 / 4.0];

        Primitive::new(gl, &quad)
    }

    fn create_title_bar(gl: GL) -> Primitive {
        let mut quad = Geometry::<Vertex>::quad();

        quad.vertices[0].uv = [0.0, 0.0 / 4.0];
        quad.vertices[1].uv = [1.0, 0.0 / 4.0];
        quad.vertices[2].uv = [1.0, 1.0 / 4.0];
        quad.vertices[3].uv = [0.0, 1.0 / 4.0];

        Primitive::new(gl, &quad)
    }

    fn create_shadow(gl: GL) -> Primitive {
        let mut quad = Geometry::<Vertex>::quad();

        quad.vertices[0].uv = [0.0, 3.0 / 4.0];
        quad.vertices[1].uv = [1.0, 3.0 / 4.0];
        quad.vertices[2].uv = [1.0, 4.0 / 4.0];
        quad.vertices[3].uv = [0.0, 4.0 / 4.0];

        Primitive::new(gl, &quad)
    }

    pub fn new(gl: &GL, width: u32, height: u32) -> Self {
        let pipeline = GuiPipeline::new(&gl);

        let view = Isometry3::look_at_rh(
            &Point3::new(0.0, 0.0, 100.5),
            &Point3::origin(),
            &Vector3::new(0.0, 1.0, 0.0),
        )
        .to_homogeneous();

        let proj =
            nalgebra::Orthographic3::new(0.0, width as f32, height as f32, 0.0, 0.125, 101.0)
                .to_homogeneous();

        let quad = Gui::create_quad(gl.clone());
        let background = Gui::create_background(gl.clone());
        let title_bar = Gui::create_title_bar(gl.clone());
        let shadow = Gui::create_shadow(gl.clone());

        let pixels = &[
            80, 80, 80, 255, // Title color
            50, 50, 50, 255, // Body color
            50, 50, 50, 255, // Red color
            255, 255, 255, 255, // Shadow color
        ];
        let image = Image::from_raw(pixels, 1, 4);
        let texture = Texture::from_image(gl.clone(), &image);

        let font = Font::new(gl.clone());

        let margin = 3;
        let title_height = font.tile_height + margin * 2;

        Self {
            width,
            height,
            pipeline,
            view,
            proj,
            quad,
            background,
            title_bar,
            shadow,
            texture,
            windows: vec![],
            windows_order: vec![],
            font,
            title_height,
            focus: None,
            dragging: false,
        }
    }

    pub fn add_window(&mut self, window: Window) {
        self.windows_order.push(self.windows.len());
        self.windows.push(window);
    }

    // Returns whether input has been handled or not
    pub fn handle_mouse(&mut self, mouse: &Mouse) -> bool {
        let mut handled = false;

        let mouse_x = mouse.pos.x;
        let mouse_y = self.height as i32 - mouse.pos.y;

        let mut new_focus = None;

        let window_count = self.windows.len();

        if mouse.left_click {
            // Iterate through windows from closer to further
            for (i, window_index) in self.windows_order.iter().rev().enumerate() {
                let window = &self.windows[*window_index];

                // Check whether mouse is clicking on the title of the window
                if !self.dragging
                    && window.title_contains(self.title_height as i32, mouse_x, mouse_y)
                {
                    self.dragging = true;
                }

                // Check whether mouse is clicking on the window
                if window.contains(mouse_x, mouse_y) {
                    if new_focus.is_none() {
                        // Set this window as focus
                        new_focus = Some(window_count - 1 - i);
                    }

                    handled = true;
                }
            }

            if new_focus != self.focus {
                if let Some(new_index) = new_focus {
                    let last_index = window_count - 1;
                    // Move new focused window at the end of the window list
                    self.windows_order.swap(new_index, last_index);
                    self.focus = Some(last_index);
                } else {
                    self.focus = None;
                }
            }
        }

        // Mouse release means not dragging anymore
        if !mouse.left_down {
            self.dragging = false;
        }

        // Update window position that we are dragging
        if self.dragging && window_count > 0 {
            let window = &mut self.windows[*self.windows_order.last().unwrap()];
            window.pos.x += mouse.drag.x;
            window.pos.y -= mouse.drag.y;
        }

        handled
    }

    pub fn draw(&self) {
        self.pipeline.program.gl.clear(GL::DEPTH_BUFFER_BIT);
        self.pipeline.program.bind();

        self.pipeline.set_view(&self.view);
        self.pipeline.set_proj(&self.proj);

        for (i, window_index) in self.windows_order.iter().enumerate() {
            let window = &self.windows[*window_index];
            self.draw_window(window, i);
        }
    }

    fn draw_window(&self, window: &Window, i: usize) {
        self.pipeline.set_sampler(0);

        self.texture.bind();

        self.pipeline.set_color(&[1.0, 1.0, 1.0, 1.0]);

        let z = i as f32;

        let is_focused = if let Some(focus_index) = self.focus {
            i == focus_index
        } else {
            false
        };

        if is_focused {
            self.pipeline.set_color(&[1.0, 1.0, 1.0, 1.0]);
        } else {
            self.pipeline.set_color(&[0.8, 0.8, 0.8, 1.0]);
        }

        // @todo Consider introducing constants for elements Z offsets

        // Title bar
        let transform = Matrix4::identity()
            .append_nonuniform_scaling(&Vector3::new(
                window.width as f32,
                self.title_height as f32,
                0.0,
            ))
            .append_translation(&Vector3::new(
                window.pos.x as f32,
                window.pos.y as f32,
                z + 0.2,
            ));
        self.pipeline.set_transform(&transform);
        self.pipeline.draw(&self.title_bar);

        self.pipeline.set_color(&[1.0, 1.0, 1.0, 1.0]);

        // Background
        let transform = Matrix4::identity()
            .append_nonuniform_scaling(&Vector3::new(
                window.width as f32,
                window.height as f32,
                0.0,
            ))
            .append_translation(&Vector3::new(
                window.pos.x as f32,
                window.pos.y as f32,
                z + 0.1,
            ));
        self.pipeline.set_transform(&transform);
        self.pipeline.draw(&self.background);

        if is_focused {
            self.pipeline.set_color(&[0.8, 0.3, 0.0, 0.4]);
        } else {
            self.pipeline.set_color(&[0.0, 0.0, 0.0, 0.4]);
        }

        // Shadow
        let transform = Matrix4::identity()
            .append_nonuniform_scaling(&Vector3::new(
                window.width as f32 + 2.0,
                window.height as f32 + 2.0,
                0.0,
            ))
            .append_translation(&Vector3::new(
                window.pos.x as f32 - 1.0,
                window.pos.y as f32 - 1.0,
                z,
            ));
        self.pipeline.set_transform(&transform);
        self.pipeline.draw(&self.shadow);

        // Text
        self.font.texture.bind();
        self.pipeline.set_color(&[1.0, 1.0, 1.0, 1.0]);

        // Draw window title name
        for (i, c) in window.name.chars().enumerate() {
            let transform = Matrix4::identity()
                .append_nonuniform_scaling(&Vector3::new(
                    self.font.tile_width as f32,
                    self.font.tile_height as f32,
                    0.0,
                ))
                .append_translation(&Vector3::new(
                    window.pos.x as f32 + 4.0 + (self.font.tile_width as usize * i) as f32,
                    window.pos.y as f32 + 4.0,
                    z + 0.3,
                ));
            self.pipeline.set_transform(&transform);

            self.pipeline.draw_char(&self.font.primitive, c);
        }

        // Draw Content

        // @todo Extract to function
        if let Some(text) = window.text.as_ref() {
            // Draw window text content
            let mut current_line_x = 0;
            let mut current_line_space_offset = 0;
            let mut offset_y = 0;

            for (i, word) in text.split(" ").enumerate() {
                let word_len = self.font.tile_width * word.len() as u32;
                current_line_x += word_len;

                let word_end_x = current_line_x + current_line_space_offset + self.font.tile_width;
                let content_size = window.width - Window::MARGIN * 2;
                if word_end_x > content_size {
                    current_line_x = word_len;
                    current_line_space_offset = 0;
                    offset_y += self.font.tile_height;
                } else if i > 0 {
                    current_line_space_offset += self.font.tile_width;
                }

                for (j, c) in word.chars().enumerate() {
                    if c == '\n' {
                        current_line_x = word_len - (1 + j as u32) * self.font.tile_width;
                        current_line_space_offset = 0;
                        offset_y += self.font.tile_height;
                        continue;
                    }
                    let current_char_x = current_line_space_offset
                        + (current_line_x - word_len)
                        + self.font.tile_width * j as u32;
                    let translation_x = window.pos.x + (Window::MARGIN + current_char_x) as i32;
                    let translation_y =
                        window.pos.y + (Window::MARGIN + self.title_height + offset_y) as i32;

                    let transform = Matrix4::identity()
                        .append_nonuniform_scaling(&Vector3::new(
                            self.font.tile_width as f32,
                            self.font.tile_height as f32,
                            0.0,
                        ))
                        .append_translation(&Vector3::new(
                            translation_x as f32,
                            translation_y as f32,
                            z + 0.3,
                        ));
                    self.pipeline.set_transform(&transform);

                    self.pipeline.draw_char(&self.font.primitive, c);
                }
            }
        }

        // Draw image
        if let Some(image) = window.image.as_ref() {
            self.draw_image(window, z, image);
        }
    }

    fn draw_image(&self, window: &Window, z: f32, image: &GuiImage) {
        self.pipeline.set_sampler(0);

        //self.pipeline.program.gl.active_texture(GL::TEXTURE0);
        self.pipeline.program.gl.bind_texture(GL::TEXTURE_2D, Some(&image.texture));

        self.pipeline.set_color(&[1.0, 1.0, 1.0, 1.0]);
        // @todo Consider refactoring either window margin or title height
        // Image
        let transform = Matrix4::identity()
            .append_nonuniform_scaling(&Vector3::new(
                (window.width - Window::MARGIN * 2) as f32,
                (window.height - Window::MARGIN * 2 - self.title_height) as f32,
                0.0,
            ))
            .append_translation(&Vector3::new(
                (window.pos.x + Window::MARGIN as i32) as f32,
                (window.pos.y + (self.title_height + Window::MARGIN) as i32) as f32,
                z + 0.3,
            ));
        self.pipeline.set_transform(&transform);
        self.pipeline.draw(&self.quad);
    }
}

pub struct Text {
    pub value: String,
}

impl Text {
    pub fn new() -> Self {
        Self {
            value: String::new(),
        }
    }
}

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Self {
            value: String::from(value),
        }
    }
}

impl Deref for Text {
    type Target = String;

    fn deref(&self) -> &String {
        &self.value
    }
}

impl DerefMut for Text {
    fn deref_mut(&mut self) -> &mut String {
        &mut self.value
    }
}

trait Element {
    /// Draw a GUI element within a window
    /// @param gui Object responsible to actually draw the element
    fn draw(&self, gui: &Gui, window: &Window, z: f32);
}

/// @todo Consider removing the wrapper
pub struct GuiImage {
    texture: WebGlTexture,
}

impl GuiImage {
    pub fn new(texture: WebGlTexture) -> Self {
        Self { texture }
    }
}

impl Element for GuiImage {
    fn draw(&self, gui: &Gui, window: &Window, z: f32) {
        // @todo Draw the image at window content position
        // Consider that texture origin is bottom-left
        // While window content position is at the top-left of the window
        gui.draw_image(window, z, self);
    }
}

pub struct Window {
    width: u32,
    height: u32,
    pos: na::Vector2<i32>,
    pub name: String,

    // @todo Figure out how to use Option<dyn Element> here
    pub text: Option<Text>,
    pub image: Option<GuiImage>,
}

impl Window {
    pub const MIN_WIDTH: u32 = 128;
    pub const MIN_HEIGHT: u32 = 128;
    pub const MARGIN: u32 = 4;

    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width: std::cmp::max(width, Self::MIN_WIDTH),
            height: std::cmp::max(height, Self::MIN_HEIGHT),
            pos: na::Vector2::new(10, 10),
            name: String::from("Test window"),
            text: None,
            image: None,
        }
    }

    /// Checks whether the specified coords are inside the whole window
    pub fn contains(&self, x: i32, y: i32) -> bool {
        let x_in_window = x > self.pos.x && x < self.pos.x + self.width as i32;
        let y_in_window = y > self.pos.y && y < self.pos.y + self.height as i32;
        x_in_window && y_in_window
    }

    /// Checks whether the specified coords are inside the title bar
    pub fn title_contains(&self, height: i32, x: i32, y: i32) -> bool {
        let x_in_title = x > self.pos.x && x < self.pos.x + self.width as i32;
        let y_in_title = y > self.pos.y && y < self.pos.y + height;
        x_in_title && y_in_title
    }

    pub fn get_width(&self) -> u32 {
        self.width
    }

    pub fn set_width(&mut self, width: u32) {
        self.width = std::cmp::max(Self::MIN_WIDTH, width);
    }

    pub fn get_height(&self) -> u32 {
        self.height
    }

    pub fn set_height(&mut self, height: u32) {
        self.height = std::cmp::max(Self::MIN_HEIGHT, height);
    }
}

#[repr(C)]
struct FontVertex {
    position: [f32; 3],
}

impl Geometry<FontVertex> {
    fn quad() -> Self {
        let vertices: Vec<FontVertex> = vec![
            // Bottom-left
            FontVertex {
                position: [0.0, 1.0, 0.0],
            },
            // Bottom-right
            FontVertex {
                position: [1.0, 1.0, 0.0],
            },
            // Top-right
            FontVertex {
                position: [1.0, 0.0, 0.0],
            },
            // Top-left
            FontVertex {
                position: [0.0, 0.0, 0.0],
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        Self { vertices, indices }
    }
}

struct Font {
    texture: Texture,
    tile_width: u32,
    tile_height: u32,
    /// This primitive is using a special vertex buffer with 4 vertices
    /// (pos, color) at the beginning followed by the UVs for all letters.
    primitive: Primitive,
}

impl Font {
    fn create_uvs(
        image_width: u32,
        image_height: u32,
        tile_width: u32,
        tile_height: u32,
    ) -> Vec<UV> {
        let row_count = image_height / tile_height;
        let column_count = image_width / tile_width;

        let expected_column_count = 32;
        assert!(column_count >= expected_column_count);

        let mut uvs: Vec<UV> = vec![];
        uvs.reserve((row_count * expected_column_count * 4) as usize);

        for i in 0..row_count {
            for j in 0..expected_column_count {
                // 4 UVs

                // Bottom-left
                uvs.push([
                    (j * tile_width) as f32 / image_width as f32,
                    (i * tile_height + tile_height) as f32 / image_height as f32,
                ]);
                // Bottom-right
                uvs.push([
                    (j * tile_width + tile_width) as f32 / image_width as f32,
                    (i * tile_height + tile_height) as f32 / image_height as f32,
                ]);
                // Top-right
                uvs.push([
                    (j * tile_width + tile_width) as f32 / image_width as f32,
                    (i * tile_height) as f32 / image_height as f32,
                ]);
                // Top-left
                uvs.push([
                    (j * tile_width) as f32 / image_width as f32,
                    (i * tile_height) as f32 / image_height as f32,
                ]);
            }
        }

        uvs
    }

    pub fn new(gl: GL) -> Self {
        let data = include_bytes!("../res/font/spd.png");
        let image = Image::from_png(data);

        let texture = Texture::from_image(gl.clone(), &image);

        let tile_width = 8;
        let tile_height = 13;
        let uvs = Font::create_uvs(image.width, image.height, tile_width, tile_height);
        let uvs_size = uvs.len() * std::mem::size_of::<UV>();

        // Make a quad with vertices with no UVs
        let quad = Geometry::<FontVertex>::quad();
        let vertices_size = quad.vertices.len() * std::mem::size_of::<FontVertex>();

        // Make a vertex buffer with 4 (position,color) at the beginning, and then all the various UVs
        let mut vertex_buffer = Vec::<u8>::new();
        vertex_buffer.resize(vertices_size + uvs_size, 0);

        // Split it
        let (vb_vertices, vb_uvs) = vertex_buffer.split_at_mut(vertices_size);

        // Copy vertices
        let vertex_buf = unsafe {
            std::slice::from_raw_parts(quad.vertices.as_ptr() as *const u8, vertices_size)
        };
        vb_vertices.copy_from_slice(vertex_buf);

        // Then copy UVs
        let uvs_buf = unsafe { std::slice::from_raw_parts(uvs.as_ptr() as *const u8, uvs_size) };
        vb_uvs.copy_from_slice(uvs_buf);

        let primitive = Primitive::from_raw(gl.clone(), &vertex_buffer, &quad.indices);

        Self {
            texture,
            tile_width,
            tile_height,
            primitive,
        }
    }
}
