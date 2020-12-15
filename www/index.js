import * as webgl from "webgl";

var ctx = null;
var teaser_ctx = webgl.Context.new_teaser();

const tick_teaser = () => {
    teaser_ctx.draw_teaser();
    requestAnimationFrame(tick_teaser);
}

requestAnimationFrame(tick_teaser);

const tick = () => {
    ctx.draw();
    requestAnimationFrame(tick);
}

var button = document.getElementById("start")
button.addEventListener("click", start_demo)

function start_demo() {
    if (ctx == null) {
        ctx = webgl.Context.new();
        button.style.display = "none"
    }
    requestAnimationFrame(tick);
}
