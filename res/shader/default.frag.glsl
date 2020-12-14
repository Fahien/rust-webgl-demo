precision mediump float;

varying vec3 position;
varying vec4 color;
varying vec3 normal;
varying vec2 uv;

uniform vec4 select_color;
uniform sampler2D tex_sampler;
uniform vec3 light_color;
uniform vec3 light_position;

void main() {
    vec3 light_direction = light_position - position;
    float n_dot_l = max(
        dot(
            normalize(light_direction),
            normalize(normal)
        ),
        0.0
    );
    vec3 diffuse = light_color * vec3(color) * n_dot_l;
    vec3 ambient = light_color * vec3(color) * 0.1;
    gl_FragColor = select_color + vec4(diffuse + ambient, color.a) * texture2D(tex_sampler, uv);
}
