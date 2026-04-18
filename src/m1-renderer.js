// M1 minimal renderer: load idle-follow SVG, track mouse within window.
// Full renderer.js (state machine, animation cycle, theme loader) comes in later milestones.

const clawdEl = document.getElementById("clawd");

const MAX_OFFSET = 3;
const BODY_SCALE = 0.33;
const SHADOW_STRETCH = 0.15;
const SHADOW_SHIFT = 0.3;

const IDS = {
  eyes: "eyes-js",
  body: "body-js",
  shadow: "shadow-js",
};

let svgDoc = null;
let eyesEl = null;
let bodyEl = null;
let shadowEl = null;

clawdEl.addEventListener("load", () => {
  svgDoc = clawdEl.contentDocument;
  if (!svgDoc) {
    console.warn("[m1] SVG contentDocument null — CSP or load failure");
    return;
  }
  eyesEl = svgDoc.getElementById(IDS.eyes);
  bodyEl = svgDoc.getElementById(IDS.body);
  shadowEl = svgDoc.getElementById(IDS.shadow);
  console.log("[m1] SVG loaded", { eyes: !!eyesEl, body: !!bodyEl, shadow: !!shadowEl });
});

function applyEyeOffset(dx, dy) {
  if (eyesEl) eyesEl.setAttribute("transform", `translate(${dx} ${dy})`);
  if (bodyEl) bodyEl.setAttribute("transform", `translate(${dx * BODY_SCALE} ${dy * BODY_SCALE})`);
  if (shadowEl) {
    const sx = 1 + Math.abs(dx) * SHADOW_STRETCH / MAX_OFFSET;
    const tx = dx * SHADOW_SHIFT;
    shadowEl.setAttribute("transform", `translate(${tx} 0) scale(${sx} 1)`);
  }
}

window.addEventListener("mousemove", (ev) => {
  const rect = document.body.getBoundingClientRect();
  const cx = rect.width / 2;
  const cy = rect.height / 2;
  const nx = Math.max(-1, Math.min(1, (ev.clientX - cx) / cx));
  const ny = Math.max(-1, Math.min(1, (ev.clientY - cy) / cy));
  applyEyeOffset(nx * MAX_OFFSET, ny * MAX_OFFSET);
});

// M2: listen for state events from Rust HTTP server
if (window.__TAURI__ && window.__TAURI__.event) {
  window.__TAURI__.event.listen("state-change", (evt) => {
    console.log("[m2] state-change", evt.payload);
  });
  window.__TAURI__.event.listen("permission-request", (evt) => {
    console.log("[m2] permission-request", evt.payload);
  });
  console.log("[m2] Tauri event listeners registered");
} else {
  console.warn("[m2] window.__TAURI__ not available — set withGlobalTauri:true");
}
