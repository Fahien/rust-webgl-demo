precision mediump float;

varying vec2 uv;

uniform vec4 color;
uniform sampler2D tex_sampler;

void main() {
    gl_FragColor = color * texture2D(tex_sampler, uv);
}
