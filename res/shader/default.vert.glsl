attribute vec3 in_position;
attribute vec4 in_color;
attribute vec3 in_normal;
attribute vec2 in_uv;

varying vec3 position;
varying vec4 color;
varying vec3 normal;
varying vec2 uv;

uniform mat4 transform;
uniform mat4 normal_transform;
uniform mat4 view;
uniform mat4 proj;

void main() {
    uv = in_uv;
    vec4 pos4 = view * transform * vec4(in_position, 1.0);
    position = pos4.xyz;
    gl_Position = proj * pos4;
    normal = mat3(normal_transform) * normalize(in_normal);
    color = in_color;
}
