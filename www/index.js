import * as webgl from "webgl";

var ctx = webgl.Context.new();

const tick = () => {
    ctx.draw();
    requestAnimationFrame(tick);
}

requestAnimationFrame(tick);
