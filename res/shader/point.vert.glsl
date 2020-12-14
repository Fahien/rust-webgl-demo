attribute vec2 position;
attribute float point_size;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    gl_PointSize = point_size;
}
