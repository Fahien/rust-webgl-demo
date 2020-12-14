attribute vec3 in_position;
attribute vec2 in_uv;

varying vec2 uv;

uniform mat4 transform;
uniform mat4 view;
uniform mat4 proj;

void main() {
    uv = in_uv;
    gl_Position = proj * view * transform * vec4(in_position, 1.0);
}
