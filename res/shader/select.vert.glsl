attribute vec3 in_position;

uniform mat4 transform;
uniform mat4 view;
uniform mat4 proj;

void main() {
    gl_Position = proj * view * transform * vec4(in_position, 1.0);
}
