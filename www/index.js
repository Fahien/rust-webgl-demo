import * as webgl from "webgl";

var slideshow = remark.create();

slideshow.on("afterShowSlide", function(slide) {
    if (slide.getSlideIndex() == 5) {
        start_demo();
    }
    if (slide.getSlideIndex() == 0) {
        start_teaser();
    }
})

var ctx = null;
var teaser_ctx = null;


const tick_teaser = () => {
    teaser_ctx.draw_teaser();
    requestAnimationFrame(tick_teaser);
}

function start_teaser() {
    if (teaser_ctx == null) {
        teaser_ctx = webgl.Context.new_teaser();
    }
    requestAnimationFrame(tick_teaser);
}

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

if (slideshow.getCurrentSlideIndex() == 0) {
    start_teaser();
} else if (slideshow.getCurrentSlideIndex() == 5) {
    start_demo();
}

