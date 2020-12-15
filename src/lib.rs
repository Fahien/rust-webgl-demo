mod utils;

use std::{cell::RefCell, rc::Rc};

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use na::{Isometry3, Point3, Translation3, UnitQuaternion, Vector2, Vector3};
use nalgebra as na;
use web_sys::WebGlRenderingContext as GL;
use web_sys::*;

mod gui;
use gui::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern "C" {
    // Use `js_namespace` here to bind `console.log(..)` instead of just
    // `log(..)`
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Wrap web-sys console log function in a println! style macro
macro_rules! log {
    ( $( $t:tt )* ) => {
        log(&format!( $( $t )* ));
    }
}

/// Returns a WebGL Context
fn get_canvas(id: &str) -> Result<HtmlCanvasElement, JsValue> {
    utils::set_panic_hook();

    let doc = window().unwrap().document().unwrap();
    let canvas = doc
        .get_element_by_id(id)
        .expect(&format!("Failed to get canvas: {}", id));
    let canvas: HtmlCanvasElement = canvas.dyn_into::<HtmlCanvasElement>()?;
    canvas.set_width(canvas.client_width() as u32);
    canvas.set_height(canvas.client_height() as u32);

    Ok(canvas)
}

fn get_gl_context(canvas: &HtmlCanvasElement) -> Result<GL, JsValue> {
    Ok(canvas.get_context("webgl")?.unwrap().dyn_into::<GL>()?)
}

/// Short WebGL program which simply clears a drawing area specified by a canvas tag
#[wasm_bindgen]
pub fn clear_drawing_area() -> Result<(), JsValue> {
    let canvas = get_canvas("area").unwrap();
    let gl = get_gl_context(&canvas)?;

    gl.clear_color(0.0, 0.0, 0.0, 1.0);
    gl.clear(GL::COLOR_BUFFER_BIT);

    Ok(())
}

/// Compiles source code into a shader object
fn compile_shader(gl: &GL, shader_type: u32, source: &str) -> WebGlShader {
    let shader = gl.create_shader(shader_type).unwrap();
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if !gl
        .get_shader_parameter(&shader, GL::COMPILE_STATUS)
        .as_bool()
        .unwrap()
    {
        let msg = gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown error"));
        panic!("Failed to compile shader: {}", msg);
    }

    shader
}

/// Links vertex and fragment shader into a shader program
fn link_program(gl: &GL, vert: WebGlShader, frag: WebGlShader) -> WebGlProgram {
    let program = gl.create_program().unwrap();

    gl.attach_shader(&program, &vert);
    gl.attach_shader(&program, &frag);
    gl.link_program(&program);

    if !gl
        .get_program_parameter(&program, GL::LINK_STATUS)
        .as_bool()
        .unwrap()
    {
        let msg = gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| String::from("Unknown error"));
        panic!("Failed to link program: {}", msg);
    }

    gl.delete_shader(Some(&vert));
    gl.delete_shader(Some(&frag));

    program
}

struct Program {
    gl: GL,
    program: WebGlProgram,
}

impl Program {
    fn new(gl: GL, vert_src: &str, frag_src: &str) -> Self {
        let vert_shader = compile_shader(&gl, GL::VERTEX_SHADER, vert_src);
        let frag_shader = compile_shader(&gl, GL::FRAGMENT_SHADER, frag_src);

        let program = link_program(&gl, vert_shader, frag_shader);

        Self { gl, program }
    }

    fn bind(&self) {
        self.gl.use_program(Some(&self.program));
    }

    fn get_attrib_loc(&self, name: &str) -> i32 {
        self.gl.get_attrib_location(&self.program, name)
    }

    fn get_uniform_loc(&self, name: &str) -> Option<WebGlUniformLocation> {
        self.gl.get_uniform_location(&self.program, name)
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        self.gl.delete_program(Some(&self.program));
    }
}

struct PointPipeline {
    program: Program,
    position_loc: i32,
    point_size_loc: i32,
    color_loc: Option<WebGlUniformLocation>,
}

impl PointPipeline {
    fn new(gl: &GL, vert_src: &str, frag_src: &str) -> Self {
        let program = Program::new(gl.clone(), vert_src, frag_src);
        program.bind();

        let position_loc = program.get_attrib_loc("position");
        let point_size_loc = program.get_attrib_loc("point_size");
        let color_loc = program.get_uniform_loc("color");

        Self {
            program,
            position_loc,
            point_size_loc,
            color_loc,
        }
    }
}

struct DefaultPipeline {
    program: Program,
    transform_loc: Option<WebGlUniformLocation>,
    normal_transform_loc: Option<WebGlUniformLocation>,
}

impl DefaultPipeline {
    fn new(gl: &GL, vert_src: &str, frag_src: &str) -> Self {
        let program = Program::new(gl.clone(), vert_src, frag_src);
        program.bind();

        let transform_loc = program.get_uniform_loc("transform");
        let normal_transform_loc = program.get_uniform_loc("normal_transform");

        Self {
            program,
            transform_loc,
            normal_transform_loc,
        }
    }

    fn bind_attribs(&self) {
        // Position
        let position_loc = self.program.get_attrib_loc("in_position");

        // Number of bytes between each vertex element
        let stride = std::mem::size_of::<Vertex>() as i32;
        // Offset of vertex data from the beginning of the buffer
        let offset = 0;

        self.program.gl.vertex_attrib_pointer_with_i32(
            position_loc as u32,
            3,
            GL::FLOAT,
            false,
            stride,
            offset,
        );
        self.program
            .gl
            .enable_vertex_attrib_array(position_loc as u32);

        // Color
        let color_loc = self.program.get_attrib_loc("in_color");

        let offset = 3 * std::mem::size_of::<f32>() as i32;
        self.program.gl.vertex_attrib_pointer_with_i32(
            color_loc as u32,
            4,
            GL::FLOAT,
            false,
            stride,
            offset,
        );
        self.program.gl.enable_vertex_attrib_array(color_loc as u32);

        // Normal
        let normal_loc = self.program.get_attrib_loc("in_normal");

        let offset = 7 * std::mem::size_of::<f32>() as i32;
        self.program.gl.vertex_attrib_pointer_with_i32(
            normal_loc as u32,
            3,
            GL::FLOAT,
            false,
            stride,
            offset,
        );
        self.program
            .gl
            .enable_vertex_attrib_array(normal_loc as u32);

        // Texture coordinates
        let uv_loc = self.program.get_attrib_loc("in_uv");
        let offset = 10 * std::mem::size_of::<f32>() as i32;
        self.program.gl.vertex_attrib_pointer_with_i32(
            uv_loc as u32,
            2,
            GL::FLOAT,
            false,
            stride,
            offset,
        );
        self.program.gl.enable_vertex_attrib_array(uv_loc as u32);
    }
}

type Color = [u8; 3];
use std::collections::HashMap;

struct SelectPipeline {
    program: Program,
    transform_loc: Option<WebGlUniformLocation>,
    color_loc: Option<WebGlUniformLocation>,

    node_colors: HashMap<u32, Color>,
}

impl SelectPipeline {
    fn new(gl: &GL) -> SelectPipeline {
        let vert_src = include_str!("../res/shader/select.vert.glsl");
        let frag_src = include_str!("../res/shader/select.frag.glsl");
        let program = Program::new(gl.clone(), vert_src, frag_src);
        program.bind();

        let transform_loc = program.get_uniform_loc("transform");
        let color_loc = program.get_uniform_loc("color");

        Self {
            program,
            transform_loc,
            color_loc,
            node_colors: HashMap::new(),
        }
    }

    fn bind_attribs(&self) {
        // Position
        let position_loc = self.program.get_attrib_loc("in_position");

        // Number of bytes between each vertex element
        let stride = std::mem::size_of::<Vertex>() as i32;
        // Offset of vertex data from the beginning of the buffer
        let offset = 0;

        self.program.gl.vertex_attrib_pointer_with_i32(
            position_loc as u32,
            3,
            GL::FLOAT,
            false,
            stride,
            offset,
        );
        self.program
            .gl
            .enable_vertex_attrib_array(position_loc as u32);
    }
}

type UV = [f32; 2];

#[repr(C)]
struct Vertex {
    position: [f32; 3], // xy
    color: [f32; 4],    // rgba
    normal: [f32; 3],
    uv: UV,
}

/// CPU-side primitive geometry
struct Geometry<V> {
    vertices: Vec<V>,
    indices: Vec<u8>,
}

impl Geometry<Vertex> {
    fn triangle() -> Self {
        let vertices = vec![
            Vertex {
                position: [-0.5, -0.5, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
            },
            Vertex {
                position: [0.5, -0.5, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
            },
            Vertex {
                position: [0.0, 0.5, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.5, 1.0],
            },
        ];

        let indices = vec![0, 1, 2];

        Self { vertices, indices }
    }

    /// Constructs a unit quad centered at the origin
    /// Vertices are ordered like so: `[bottom-left, bottom-right, top-right, top-left]`
    fn quad() -> Self {
        let vertices: Vec<Vertex> = vec![
            // Bottom-left
            Vertex {
                position: [0.0, 1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
            },
            // Bottom-right
            Vertex {
                position: [1.0, 1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 1.0],
            },
            // Top-right
            Vertex {
                position: [1.0, 0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
            },
            // Top-left
            Vertex {
                position: [0.0, 0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        Self { vertices, indices }
    }

    fn cube() -> Self {
        let vertices = vec![
            // Front
            Vertex {
                position: [-0.5, -0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
            },
            Vertex {
                position: [0.5, -0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
            },
            Vertex {
                position: [0.5, 0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 1.0],
            },
            Vertex {
                position: [-0.5, 0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
            },
            // Right
            Vertex {
                position: [0.5, -0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [1.0, 0.0, 0.0],
                uv: [0.0, 0.0],
            },
            Vertex {
                position: [0.5, -0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [1.0, 0.0, 0.0],
                uv: [1.0, 0.0],
            },
            Vertex {
                position: [0.5, 0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [1.0, 0.0, 0.0],
                uv: [1.0, 1.0],
            },
            Vertex {
                position: [0.5, 0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [1.0, 0.0, 0.0],
                uv: [0.0, 1.0],
            },
            // Back
            Vertex {
                position: [0.5, -0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, -1.0],
                uv: [0.0, 0.0],
            },
            Vertex {
                position: [-0.5, -0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, -1.0],
                uv: [1.0, 0.0],
            },
            Vertex {
                position: [-0.5, 0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, -1.0],
                uv: [1.0, 1.0],
            },
            Vertex {
                position: [0.5, 0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 0.0, -1.0],
                uv: [0.0, 1.0],
            },
            // Left
            Vertex {
                position: [-0.5, -0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [-1.0, 0.0, 0.0],
                uv: [0.0, 0.0],
            },
            Vertex {
                position: [-0.5, -0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [-1.0, 0.0, 0.0],
                uv: [1.0, 0.0],
            },
            Vertex {
                position: [-0.5, 0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [-1.0, 0.0, 0.0],
                uv: [1.0, 1.0],
            },
            Vertex {
                position: [-0.5, 0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [-1.0, 0.0, 0.0],
                uv: [0.0, 1.0],
            },
            // Top
            Vertex {
                position: [-0.5, 0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
            },
            Vertex {
                position: [0.5, 0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 0.0],
            },
            Vertex {
                position: [0.5, 0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
                uv: [1.0, 1.0],
            },
            Vertex {
                position: [-0.5, 0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 1.0],
            },
            // Bottom
            Vertex {
                position: [-0.5, -0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, -1.0, 0.0],
                uv: [0.0, 0.0],
            },
            Vertex {
                position: [0.5, -0.5, -0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, -1.0, 0.0],
                uv: [1.0, 0.0],
            },
            Vertex {
                position: [0.5, -0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, -1.0, 0.0],
                uv: [1.0, 1.0],
            },
            Vertex {
                position: [-0.5, -0.5, 0.5],
                color: [1.0, 1.0, 1.0, 1.0],
                normal: [0.0, -1.0, 0.0],
                uv: [0.0, 1.0],
            },
        ];

        let indices: Vec<u8> = vec![
            0, 1, 2, 0, 2, 3, // front face
            4, 5, 6, 4, 6, 7, // right
            8, 9, 10, 8, 10, 11, // back
            12, 13, 14, 12, 14, 15, // left
            16, 17, 18, 16, 18, 19, // top
            20, 21, 22, 20, 22, 23, // bottom
        ];

        Self { vertices, indices }
    }
}

/// GPU-side primitive geometry
struct Primitive {
    gl: GL,
    vertex_buffer: Option<WebGlBuffer>,
    index_buffer: Option<WebGlBuffer>,
    index_count: i32,
}

impl Primitive {
    fn from_raw<T>(gl: GL, vertices: &Vec<T>, indices: &Vec<u8>) -> Self {
        let vertex_buffer = gl.create_buffer();
        gl.bind_buffer(GL::ARRAY_BUFFER, vertex_buffer.as_ref());
        let u8_slice = unsafe {
            std::slice::from_raw_parts(
                vertices.as_ptr() as *const u8,
                vertices.len() * std::mem::size_of::<T>(),
            )
        };
        gl.buffer_data_with_u8_array(GL::ARRAY_BUFFER, u8_slice, GL::STATIC_DRAW);

        let index_buffer = gl.create_buffer();
        gl.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, index_buffer.as_ref());
        gl.buffer_data_with_u8_array(GL::ELEMENT_ARRAY_BUFFER, &indices, GL::STATIC_DRAW);

        let index_count = indices.len() as i32;
        Self {
            gl,
            vertex_buffer,
            index_buffer,
            index_count,
        }
    }

    fn new<V>(gl: GL, geometry: &Geometry<V>) -> Self {
        Self::from_raw(gl, &geometry.vertices, &geometry.indices)
    }

    fn bind(&self) {
        self.gl
            .bind_buffer(GL::ARRAY_BUFFER, self.vertex_buffer.as_ref());
        self.gl
            .bind_buffer(GL::ELEMENT_ARRAY_BUFFER, self.index_buffer.as_ref());
    }

    fn draw(&self) {
        self.gl
            .draw_elements_with_i32(GL::TRIANGLES, self.index_count, GL::UNSIGNED_BYTE, 0);
    }
}

impl Drop for Primitive {
    fn drop(&mut self) {
        self.gl.delete_buffer(self.vertex_buffer.as_ref());
        self.gl.delete_buffer(self.index_buffer.as_ref());
    }
}

struct Image {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

impl Image {
    /// Creates a default image of one white pixel
    fn new() -> Self {
        Self {
            data: vec![255, 255, 255, 255],
            width: 1,
            height: 1,
        }
    }

    /// Creates an image from raw data
    fn from_raw(data: &[u8], width: u32, height: u32) -> Self {
        Self::from_vec(data.into(), width, height)
    }

    /// Creates an image from raw data as vector
    fn from_vec(data: Vec<u8>, width: u32, height: u32) -> Self {
        let channels = 4; // RGBA
        assert!(data.len() as u32 == width * height * channels);
        Self {
            data,
            width,
            height,
        }
    }

    /// Creates an image from png data
    fn from_png(png_data: &[u8]) -> Self {
        let decoder = png::Decoder::new(png_data);
        let (info, mut reader) = decoder.read_info().expect("Failed reading png info");
        let mut data: Vec<u8> = vec![0; info.buffer_size()];
        reader
            .next_frame(data.as_mut_slice())
            .expect("Failed to read png frame");

        Image::from_vec(data, info.width, info.height)
    }
}

pub struct Texture {
    gl: GL,
    handle: WebGlTexture,
    width: u32,
    height: u32,
}

impl Texture {
    /// Returns a new texture uploading data from the specified image
    fn from_image(gl: GL, image: &Image) -> Self {
        let handle = gl.create_texture().expect("Failed to create texture");

        let mut texture = Self {
            gl,
            handle,
            width: 0,
            height: 0,
        };

        texture.bind();

        texture
            .gl
            .tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::LINEAR as i32);
        texture
            .gl
            .tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::LINEAR as i32);

        //    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        texture
            .gl
            .tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        texture
            .gl
            .tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);

        texture.upload(Some(&image.data), image.width, image.height);

        texture
    }

    /// Returns a new default texture with a default image (2x2 red, blue, green, white)
    fn new(gl: GL) -> Self {
        let pixels = [
            255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ];
        let image = Image::from_raw(&pixels, 2, 2);
        Self::from_image(gl, &image)
    }

    fn bind(&self) {
        //self.gl.active_texture(GL::TEXTURE0);
        self.gl.bind_texture(GL::TEXTURE_2D, Some(&self.handle));
    }

    /// Uploads pixels data to the texture memory in the GPU
    fn upload(&mut self, pixels: Option<&[u8]>, width: u32, height: u32) {
        self.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                GL::TEXTURE_2D,
                0,
                GL::RGBA as i32,
                width as i32,
                height as i32,
                0,
                GL::RGBA,
                GL::UNSIGNED_BYTE,
                pixels,
            )
            .expect("Failed to upload texture data");

        self.width = width;
        self.height = height;
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        self.gl.delete_texture(Some(&self.handle))
    }
}

struct Node {
    id: u32,
    transform: Isometry3<f32>,
    primitive: Primitive,
    children: Vec<Node>,
}

impl Node {
    fn new(primitive: Primitive) -> Self {
        Self {
            id: 0,
            transform: Isometry3::identity(),
            primitive,
            children: vec![],
        }
    }
}

pub struct Mouse {
    pos: na::Vector2<i32>,
    prev: na::Vector2<i32>,
    drag: na::Vector2<i32>,

    left_click: bool,
    left_down: bool,

    selected_node: Option<u32>,
}

impl Mouse {
    const LEFT: u16 = 1;
    const RIGHT: u16 = 2;
    const MIDDLE: u16 = 4;

    fn new() -> Self {
        Self {
            pos: na::Vector2::new(0, 0),
            prev: na::Vector2::new(0, 0),
            drag: na::Vector2::new(0, 0),
            left_click: false,
            left_down: false,
            selected_node: None,
        }
    }

    fn reset(&mut self) {
        self.left_click = false;
        self.drag.x = 0;
        self.drag.y = 0;
    }
}

struct Framebuffer {
    frame: Option<WebGlFramebuffer>,
    color: Option<WebGlRenderbuffer>,
    depth: Option<WebGlRenderbuffer>,
    texture: Option<Texture>,
}

#[wasm_bindgen]
pub struct Context {
    performance: web_sys::Performance,
    canvas: HtmlCanvasElement,
    gl: WebGlRenderingContext,
    view: Rc<RefCell<Isometry3<f32>>>,
    mouse: Rc<RefCell<Mouse>>,
    offscreen_framebuffer: Framebuffer,
    point_pipeline: PointPipeline,
    default_pipeline: DefaultPipeline,
    select_pipeline: SelectPipeline,
    nodes: Vec<Node>,
    texture: Texture,

    // @todo Remove Rc<RefCell>
    gui: Rc<RefCell<Gui>>,
}

fn create_point_program(gl: &WebGlRenderingContext) -> PointPipeline {
    let vert_src = include_str!("../res/shader/point.vert.glsl");
    let frag_src = include_str!("../res/shader/point.frag.glsl");

    PointPipeline::new(gl, vert_src, frag_src)
}

fn create_default_program(gl: &WebGlRenderingContext) -> DefaultPipeline {
    let vert_src = include_str!("../res/shader/default.vert.glsl");
    let frag_src = include_str!("../res/shader/default.frag.glsl");
    DefaultPipeline::new(gl, vert_src, frag_src)
}

use rand::Rng;

fn generate_node_colors(
    select_pipeline: &mut SelectPipeline,
    rng: &mut rand::rngs::ThreadRng,
    node: &Node,
) {
    let color: Color = [rng.gen(), rng.gen(), rng.gen()];
    select_pipeline.node_colors.insert(node.id, color);

    for child in &node.children {
        generate_node_colors(select_pipeline, rng, child);
    }
}

fn create_offscreen_framebuffer(gl: &GL, width: i32, height: i32) -> Framebuffer {
    let offscreen_framebuffer = gl.create_framebuffer();
    gl.bind_framebuffer(GL::FRAMEBUFFER, offscreen_framebuffer.as_ref());

    let offscreen_colorbuffer = gl.create_renderbuffer();
    gl.bind_renderbuffer(GL::RENDERBUFFER, offscreen_colorbuffer.as_ref());
    gl.renderbuffer_storage(GL::RENDERBUFFER, GL::RGBA4, width, height);
    gl.framebuffer_renderbuffer(
        GL::FRAMEBUFFER,
        GL::COLOR_ATTACHMENT0,
        GL::RENDERBUFFER,
        offscreen_colorbuffer.as_ref(),
    );

    let offscreen_depthbuffer = gl.create_renderbuffer();
    gl.bind_renderbuffer(GL::RENDERBUFFER, offscreen_depthbuffer.as_ref());
    gl.renderbuffer_storage(GL::RENDERBUFFER, GL::DEPTH_COMPONENT16, width, height);
    gl.framebuffer_renderbuffer(
        GL::FRAMEBUFFER,
        GL::DEPTH_ATTACHMENT,
        GL::RENDERBUFFER,
        offscreen_depthbuffer.as_ref(),
    );

    gl.bind_framebuffer(GL::FRAMEBUFFER, None);

    Framebuffer {
        frame: offscreen_framebuffer,
        color: offscreen_colorbuffer,
        depth: offscreen_depthbuffer,
        texture: None,
    }
}

fn create_select_framebuffer(gl: &GL, width: i32, height: i32) -> Framebuffer {
    // @todo Create a framebuffer object
    let select_framebuffer = gl.create_framebuffer();
    gl.bind_framebuffer(GL::FRAMEBUFFER, select_framebuffer.as_ref());

    // @todo Create a texture object
    let mut texture = Texture::new(gl.clone());
    texture.upload(None, width as u32, height as u32);
    gl.bind_texture(GL::TEXTURE_2D, None);

    gl.framebuffer_texture_2d(
        GL::FRAMEBUFFER,
        GL::COLOR_ATTACHMENT0,
        GL::TEXTURE_2D,
        Some(&texture.handle),
        0,
    );

    // @todo Check error checkframebuffer

    let select_depthbuffer = gl.create_renderbuffer();
    gl.bind_renderbuffer(GL::RENDERBUFFER, select_depthbuffer.as_ref());
    gl.renderbuffer_storage(GL::RENDERBUFFER, GL::DEPTH_COMPONENT16, width, height);
    gl.framebuffer_renderbuffer(
        GL::FRAMEBUFFER,
        GL::DEPTH_ATTACHMENT,
        GL::RENDERBUFFER,
        select_depthbuffer.as_ref(),
    );

    let e = gl.check_framebuffer_status(GL::FRAMEBUFFER);
    if e != GL::FRAMEBUFFER_COMPLETE {
        log("Framebuffer error");
    }

    // Unbind
    gl.bind_framebuffer(GL::FRAMEBUFFER, None);

    Framebuffer {
        frame: select_framebuffer,
        color: None,
        depth: select_depthbuffer,
        texture: Some(texture),
    }
}

#[wasm_bindgen]
impl Context {
    pub fn new() -> Result<Context, JsValue> {
        let window = web_sys::window().unwrap();
        let performance = window.performance().unwrap();

        let canvas = get_canvas("area")?;
        let gl = get_gl_context(&canvas)?;

        //let offscreen_framebuffer =
        //create_offscreen_framebuffer(&gl, canvas.width() as i32, canvas.height() as i32);

        let select_framebuffer =
            create_select_framebuffer(&gl, canvas.width() as i32, canvas.height() as i32);

        let point_pipeline = create_point_program(&gl);
        let default_pipeline = create_default_program(&gl);
        let mut select_pipeline = SelectPipeline::new(&gl);

        // OpenGL uses a right-handed coordinate system
        let view = Rc::new(RefCell::new(Isometry3::look_at_rh(
            &Point3::new(0.0, 0.0, 6.0),
            &Point3::origin(),
            &Vector3::y_axis(),
        )));

        let mut nodes = vec![];

        let cube = Geometry::cube();

        let mut root = Node::new(Primitive::new(gl.clone(), &cube));
        root.transform
            .append_translation_mut(&Translation3::new(0.0, 0.0, 0.0));

        let mut node_right = Node::new(Primitive::new(gl.clone(), &cube));
        node_right.id = 1;
        node_right
            .transform
            .append_translation_mut(&Translation3::new(1.5, 0.0, 0.0));

        let mut node_left = Node::new(Primitive::new(gl.clone(), &cube));
        node_left.id = 2;
        node_left
            .transform
            .append_translation_mut(&Translation3::new(-1.5, 0.0, 0.0));

        root.children.push(node_right);
        root.children.push(node_left);

        // Create select color for each node
        let mut rng = rand::thread_rng();
        generate_node_colors(&mut select_pipeline, &mut rng, &root);

        nodes.push(root);

        let texture = Texture::new(gl.clone());

        // @todo Extract to function: Create GUI
        let mut gui = Gui::new(&gl, canvas.width(), canvas.height());

        let mut mouse_window = gui::Window::new(240, 32);
        mouse_window.name = String::from("Mouse state");
        mouse_window.text = Some(Text::new());
        gui.add_window(mouse_window);

        let mut image_window = gui::Window::new(200, 180);
        image_window.name = String::from("Select buffer");

        // @todo Try with a reference?
        image_window.image = Some(GuiImage::new(
            select_framebuffer.texture.as_ref().unwrap().handle.clone(),
        ));
        gui.add_window(image_window);

        let ret = Context {
            performance,
            canvas,
            gl,
            view,
            mouse: Rc::new(RefCell::new(Mouse::new())),
            offscreen_framebuffer: select_framebuffer,
            point_pipeline,
            default_pipeline,
            select_pipeline,
            nodes,
            texture,

            gui: Rc::new(RefCell::new(gui)),
        };

        let document = window.document().unwrap();
        ret.set_onmousemove(&document);
        ret.set_onwheel(&document);
        ret.set_onmousedown(&document);
        ret.set_onmouseup(&document);

        Ok(ret)
    }

    fn set_onmousedown(&self, document: &Document) {
        let mouse = self.mouse.clone();
        let callback = Box::new(move |e: web_sys::MouseEvent| {
            // Update mouse state
            let mut mouse = mouse.borrow_mut();
            if e.buttons() == Mouse::LEFT {
                mouse.left_down = true;
                mouse.left_click = true;
            }
        });
        let closure =
            wasm_bindgen::closure::Closure::wrap(callback as Box<dyn FnMut(web_sys::MouseEvent)>);
        document.set_onmousedown(Some(closure.as_ref().unchecked_ref()));
        closure.forget();
    }

    fn set_onmouseup(&self, document: &Document) {
        let mouse = self.mouse.clone();
        let callback = Box::new(move |e: web_sys::MouseEvent| {
            // Update mouse state
            let mut mouse = mouse.borrow_mut();
            if e.buttons() != Mouse::LEFT {
                mouse.left_down = false;
            }
        });
        let closure =
            wasm_bindgen::closure::Closure::wrap(callback as Box<dyn FnMut(web_sys::MouseEvent)>);
        document.set_onmouseup(Some(closure.as_ref().unchecked_ref()));
        closure.forget();
    }

    fn set_onmousemove(&self, document: &Document) {
        let view = self.view.clone();
        let mouse = self.mouse.clone();
        let height = self.canvas.height() as i32;

        let callback = Box::new(move |e: web_sys::MouseEvent| {
            // Update mouse state
            {
                let mut mouse = mouse.borrow_mut();

                mouse.prev = mouse.pos;

                mouse.pos.x = e.offset_x();
                mouse.pos.y = height - e.offset_y();

                mouse.drag.x += mouse.pos.x - mouse.prev.x;
                mouse.drag.y += mouse.pos.y - mouse.prev.y;
            }

            // Shift + Mouse Left
            if e.shift_key() && e.buttons() == Mouse::LEFT {
                // Camera panning
                let x = e.movement_x() as f32 / 256.0;
                let y = -(e.movement_y() as f32 / 256.0);
                view.borrow_mut()
                    .append_translation_mut(&Translation3::new(x, y, 0.0));
            }

            // Camera orbiting
            if e.buttons() == Mouse::MIDDLE {
                let x = e.movement_x() as f32 / 256.0;
                let y = -(e.movement_y() as f32 / 256.0);

                let rotation = UnitQuaternion::<f32>::from_axis_angle(&Vector3::y_axis(), x);
                let rotation =
                    rotation * UnitQuaternion::<f32>::from_axis_angle(&Vector3::x_axis(), y);
                view.borrow_mut().append_rotation_wrt_center_mut(&rotation);
            }
        });
        let closure =
            wasm_bindgen::closure::Closure::wrap(callback as Box<dyn FnMut(web_sys::MouseEvent)>);
        document.set_onmousemove(Some(closure.as_ref().unchecked_ref()));
        closure.forget();
    }

    fn set_onwheel(&self, document: &Document) {
        let view = self.view.clone();
        let callback = Box::new(move |e: web_sys::WheelEvent| {
            let x = -e.delta_x() as f32 / 256.0;
            let y = -e.delta_y() as f32 / 256.0;
            // Camera zoom in/out
            view.borrow_mut()
                .append_translation_mut(&Translation3::new(x, 0.0, y));
        });
        let closure =
            wasm_bindgen::closure::Closure::wrap(callback as Box<dyn FnMut(web_sys::WheelEvent)>);
        document.set_onwheel(Some(closure.as_ref().unchecked_ref()));
        closure.forget();
    }

    /// Draws a point at position x and y
    pub fn draw_point(&self, x: f32, y: f32) -> Result<(), JsValue> {
        self.point_pipeline.program.bind();

        self.gl
            .vertex_attrib1f(self.point_pipeline.point_size_loc as u32, 16.0);
        self.gl
            .vertex_attrib3f(self.point_pipeline.position_loc as u32, x, y, 0.0);
        self.gl
            .uniform4f(self.point_pipeline.color_loc.as_ref(), 0.0, 1.0, 0.0, 1.0);

        self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
        self.gl.clear(GL::COLOR_BUFFER_BIT);

        self.gl.draw_arrays(GL::POINTS, 0, 1);

        Ok(())
    }

    // Update window content
    fn update_gui(&self) {
        let focus = self.gui.borrow().focus.clone();
        let window = &mut self.gui.borrow_mut().windows[0];
        let mouse = self.mouse.borrow();

        if let Some(text) = window.text.as_mut() {
            text.value = format!(
                "Mouse
 pos: ({},{})
 drag: ({},{})
 left: (click: {}, down: {})
 focus: {:?}",
                mouse.pos.x,
                mouse.pos.y,
                mouse.drag.x,
                mouse.drag.y,
                mouse.left_click,
                mouse.left_down,
                focus,
            );
        }
    }

    /// Handles user input
    fn handle_input(&self) -> Result<(), JsValue> {
        let mut mouse = self.mouse.borrow_mut();

        // Check whether the GUI captures mouse input
        if self.gui.borrow_mut().handle_mouse(&mouse) {
            return Ok(());
        }

        // Selection pipeline
        if mouse.left_click {
            self.gl
                .bind_framebuffer(GL::FRAMEBUFFER, self.offscreen_framebuffer.frame.as_ref());

            self.draw_select()?;

            let mut pixel = [0u8, 0, 0, 0];
            self.gl.read_pixels_with_opt_u8_array(
                mouse.pos.x,
                mouse.pos.y,
                1,
                1,
                GL::RGBA,
                GL::UNSIGNED_BYTE,
                Some(&mut pixel),
            )?;

            self.gl.bind_framebuffer(GL::FRAMEBUFFER, None);

            for pair in self.select_pipeline.node_colors.iter() {
                let color = pair.1;
                if pixel[0] == color[0] && pixel[1] == color[1] && pixel[2] == color[2] {
                    mouse.selected_node = Some(*pair.0);
                    break;
                }

                mouse.selected_node = None;
            }
        }

        Ok(())
    }

    /// Draws the scene
    pub fn draw(&self) -> Result<(), JsValue> {
        self.handle_input()?;
        self.update_gui();
        // After using input, reset its state
        self.mouse.borrow_mut().reset();

        // Set graphics state
        self.gl.enable(GL::DEPTH_TEST);
        self.gl.enable(GL::BLEND);
        self.gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);

        self.default_pipeline.program.bind();

        // View
        let view_loc = self.default_pipeline.program.get_uniform_loc("view");

        self.gl.uniform_matrix4fv_with_f32_array(
            view_loc.as_ref(),
            false,
            self.view.borrow().to_homogeneous().as_slice(),
        );

        // Proj
        let proj_loc = self.default_pipeline.program.get_uniform_loc("proj");

        let width = self.canvas.width() as f32;
        let height = self.canvas.height() as f32;
        let proj = nalgebra::Perspective3::new(width / height, 3.14 / 4.0, 0.125, 256.0);
        self.gl.uniform_matrix4fv_with_f32_array(
            proj_loc.as_ref(),
            false,
            proj.to_homogeneous().as_slice(),
        );

        // Lighting
        let light_color_loc = self.default_pipeline.program.get_uniform_loc("light_color");
        self.gl.uniform3f(light_color_loc.as_ref(), 1.0, 1.0, 1.0);

        let light_position_loc = self
            .default_pipeline
            .program
            .get_uniform_loc("light_position");
        self.gl
            .uniform3f(light_position_loc.as_ref(), 4.0, 1.0, 1.0);

        // Texture
        self.texture.bind();
        let sampler_loc = self.default_pipeline.program.get_uniform_loc("tex_sampler");
        self.gl.uniform1i(sampler_loc.as_ref(), 0);

        self.gl.clear_color(0.9, 0.9, 0.9, 1.0);
        self.gl.clear(GL::COLOR_BUFFER_BIT);
        self.gl.clear(GL::DEPTH_BUFFER_BIT);

        // Time
        let now = self.performance.now();

        let mut transform = Isometry3::<f32>::identity();
        let rotation =
            UnitQuaternion::<f32>::from_axis_angle(&Vector3::z_axis(), now as f32 / 4096.0);
        transform.append_rotation_mut(&rotation);
        let rotation =
            UnitQuaternion::<f32>::from_axis_angle(&Vector3::y_axis(), now as f32 / 4096.0);
        transform.append_rotation_mut(&rotation);

        // Draw all nodes
        for node in &self.nodes {
            self.draw_node(now as f32, &node, &transform);
        }

        self.gui.borrow().draw();

        Ok(())
    }

    fn draw_node(&self, now: f32, node: &Node, parent_trs: &Isometry3<f32>) {
        node.primitive.bind();
        self.default_pipeline.bind_attribs();

        // Select color
        let select_color_loc = self
            .default_pipeline
            .program
            .get_uniform_loc("select_color");
        let select_color = match self.mouse.borrow().selected_node {
            Some(node_id) if node_id == node.id => [0.4f32, 0.4, 0.1, 0.0],
            _ => [0.0f32, 0.0, 0.0, 0.0],
        };
        self.gl
            .uniform4fv_with_f32_array(select_color_loc.as_ref(), &select_color);

        let transform = parent_trs * node.transform;

        self.gl.uniform_matrix4fv_with_f32_array(
            self.default_pipeline.transform_loc.as_ref(),
            false,
            transform.to_homogeneous().as_slice(),
        );

        let normal_transform = transform.inverse().to_homogeneous().transpose();
        self.gl.uniform_matrix4fv_with_f32_array(
            self.default_pipeline.normal_transform_loc.as_ref(),
            false,
            normal_transform.as_slice(),
        );

        node.primitive.draw();

        for child in &node.children {
            self.draw_node(now, child, &transform);
        }
    }

    /// Draw the scene with the select pipeline
    pub fn draw_select(&self) -> Result<(), JsValue> {
        self.gl.enable(GL::DEPTH_TEST);
        self.select_pipeline.program.bind();

        // View
        let view_loc = self.select_pipeline.program.get_uniform_loc("view");

        self.gl.uniform_matrix4fv_with_f32_array(
            view_loc.as_ref(),
            false,
            self.view.borrow().to_homogeneous().as_slice(),
        );

        // Proj
        let proj_loc = self.select_pipeline.program.get_uniform_loc("proj");

        let width = self.canvas.width() as f32;
        let height = self.canvas.height() as f32;
        let proj = nalgebra::Perspective3::new(width / height, 3.14 / 4.0, 0.125, 256.0);
        self.gl.uniform_matrix4fv_with_f32_array(
            proj_loc.as_ref(),
            false,
            proj.to_homogeneous().as_slice(),
        );

        // Time
        let now = self.performance.now();

        let mut transform = Isometry3::<f32>::identity();
        let rotation =
            UnitQuaternion::<f32>::from_axis_angle(&Vector3::z_axis(), now as f32 / 4096.0);
        transform.append_rotation_mut(&rotation);
        let rotation =
            UnitQuaternion::<f32>::from_axis_angle(&Vector3::y_axis(), now as f32 / 4096.0);
        transform.append_rotation_mut(&rotation);

        // Clear framebuffer
        self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
        self.gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT);

        // Draw all nodes
        for node in &self.nodes {
            self.draw_select_node(now as f32, &node, &transform);
        }

        Ok(())
    }

    fn draw_select_node(&self, now: f32, node: &Node, parent_trs: &Isometry3<f32>) {
        node.primitive.bind();
        self.select_pipeline.bind_attribs();

        // Color
        let color = self
            .select_pipeline
            .node_colors
            .get(&node.id)
            .expect(&format!("Failed to get select color for node {}", node.id));
        let color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
        ];
        self.gl
            .uniform3fv_with_f32_array(self.select_pipeline.color_loc.as_ref(), &color);

        // Transform
        let transform = parent_trs * node.transform;

        self.gl.uniform_matrix4fv_with_f32_array(
            self.select_pipeline.transform_loc.as_ref(),
            false,
            transform.to_homogeneous().as_slice(),
        );

        // Draw call
        node.primitive.draw();

        // Recursively draw this node's children
        for child in &node.children {
            self.draw_select_node(now, child, &transform);
        }
    }

    pub fn new_teaser() -> Result<Context, JsValue> {
        let window = web_sys::window().unwrap();
        let performance = window.performance().unwrap();

        let canvas = get_canvas("teaser")?;
        let gl = get_gl_context(&canvas)?;

        //let offscreen_framebuffer =
        //create_offscreen_framebuffer(&gl, canvas.width() as i32, canvas.height() as i32);

        let select_framebuffer =
            create_select_framebuffer(&gl, canvas.width() as i32, canvas.height() as i32);

        let point_pipeline = create_point_program(&gl);
        let default_pipeline = create_default_program(&gl);
        let mut select_pipeline = SelectPipeline::new(&gl);

        // OpenGL uses a right-handed coordinate system
        let view = Rc::new(RefCell::new(Isometry3::look_at_rh(
            &Point3::new(0.0, 0.0, 3.0),
            &Point3::origin(),
            &Vector3::y_axis(),
        )));

        let mut nodes = vec![];

        let cube = Geometry::cube();

        let mut root = Node::new(Primitive::new(gl.clone(), &cube));
        root.transform
            .append_translation_mut(&Translation3::new(0.0, 0.0, 0.0));

        // Create select color for each node
        let mut rng = rand::thread_rng();
        generate_node_colors(&mut select_pipeline, &mut rng, &root);

        nodes.push(root);

        let texture = Texture::new(gl.clone());

        // @todo Extract to function: Create GUI
        let gui = Gui::new(&gl, canvas.width(), canvas.height());

        let ret = Context {
            performance,
            canvas,
            gl,
            view,
            mouse: Rc::new(RefCell::new(Mouse::new())),
            offscreen_framebuffer: select_framebuffer,
            point_pipeline,
            default_pipeline,
            select_pipeline,
            nodes,
            texture,

            gui: Rc::new(RefCell::new(gui)),
        };

        Ok(ret)
    }

    /// Draws the scene
    pub fn draw_teaser(&self) -> Result<(), JsValue> {
        //self.handle_input()?;
        // After using input, reset its state
        self.mouse.borrow_mut().reset();

        // Set graphics state
        self.gl.enable(GL::DEPTH_TEST);
        self.gl.enable(GL::BLEND);
        self.gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);

        self.default_pipeline.program.bind();

        // View
        let view_loc = self.default_pipeline.program.get_uniform_loc("view");

        self.gl.uniform_matrix4fv_with_f32_array(
            view_loc.as_ref(),
            false,
            self.view.borrow().to_homogeneous().as_slice(),
        );

        // Proj
        let proj_loc = self.default_pipeline.program.get_uniform_loc("proj");

        let width = self.canvas.width() as f32;
        let height = self.canvas.height() as f32;
        let proj = nalgebra::Perspective3::new(width / height, 3.14 / 4.0, 0.125, 256.0);
        self.gl.uniform_matrix4fv_with_f32_array(
            proj_loc.as_ref(),
            false,
            proj.to_homogeneous().as_slice(),
        );

        // Lighting
        let light_color_loc = self.default_pipeline.program.get_uniform_loc("light_color");
        self.gl.uniform3f(light_color_loc.as_ref(), 1.0, 1.0, 1.0);

        let light_position_loc = self
            .default_pipeline
            .program
            .get_uniform_loc("light_position");
        self.gl
            .uniform3f(light_position_loc.as_ref(), 4.0, 1.0, 1.0);

        // Texture
        self.texture.bind();
        let sampler_loc = self.default_pipeline.program.get_uniform_loc("tex_sampler");
        self.gl.uniform1i(sampler_loc.as_ref(), 0);

        self.gl.clear_color(1.0, 1.0, 1.0, 1.0);
        self.gl.clear(GL::COLOR_BUFFER_BIT);
        self.gl.clear(GL::DEPTH_BUFFER_BIT);

        // Time
        let now = self.performance.now();

        let mut transform = Isometry3::<f32>::identity();
        let rotation =
            UnitQuaternion::<f32>::from_axis_angle(&Vector3::z_axis(), now as f32 / 4096.0);
        transform.append_rotation_mut(&rotation);
        let rotation =
            UnitQuaternion::<f32>::from_axis_angle(&Vector3::y_axis(), now as f32 / 4096.0);
        transform.append_rotation_mut(&rotation);

        // Draw all nodes
        for node in &self.nodes {
            self.draw_node(now as f32, &node, &transform);
        }

        self.gui.borrow().draw();

        Ok(())
    }
}
