// M1/M3 renderer: SVG render + eye tracking + state-driven SVG swap.

const clawdEl = document.getElementById("clawd");

const MAX_OFFSET = 3;
const BODY_SCALE = 0.33;
const SHADOW_STRETCH = 0.15;
const SHADOW_SHIFT = 0.3;
const BASE = "assets/svg/";

const STATE_SVG = {
  idle: "clawd-idle-follow.svg",
  thinking: "clawd-working-thinking.svg",
  typing: "clawd-working-typing.svg",
  working: "clawd-working-typing.svg",
  juggling: "clawd-working-juggling.svg",
  conducting: "clawd-working-conducting.svg",
  building: "clawd-working-building.svg",
  carrying: "clawd-working-carrying.svg",
  sweeping: "clawd-working-sweeping.svg",
  attention: "clawd-happy.svg",
  happy: "clawd-happy.svg",
  notification: "clawd-notification.svg",
  error: "clawd-error.svg",
  sleeping: "clawd-sleeping.svg",
};

const EYE_TRACKING_STATES = new Set(["idle"]);

const IDS = { eyes: "eyes-js", body: "body-js", shadow: "shadow-js" };

let svgDoc = null;
let eyesEl = null;
let bodyEl = null;
let shadowEl = null;
let currentState = "idle";

function rewireSvgRefs() {
  svgDoc = clawdEl.contentDocument;
  if (!svgDoc) {
    eyesEl = bodyEl = shadowEl = null;
    return;
  }
  eyesEl = svgDoc.getElementById(IDS.eyes);
  bodyEl = svgDoc.getElementById(IDS.body);
  shadowEl = svgDoc.getElementById(IDS.shadow);
}

clawdEl.addEventListener("load", () => {
  rewireSvgRefs();
  console.log("[m1] SVG loaded", currentState, { eyes: !!eyesEl, body: !!bodyEl, shadow: !!shadowEl });
});

function swapSvg(state) {
  const file = STATE_SVG[state];
  if (!file) {
    console.warn("[m3] no SVG mapping for state:", state);
    return;
  }
  currentState = state;
  clawdEl.setAttribute("data", BASE + file);
}

function applyEyeOffset(dx, dy) {
  if (!EYE_TRACKING_STATES.has(currentState)) return;
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

if (window.__TAURI__ && window.__TAURI__.event) {
  window.__TAURI__.event.listen("state-change", (evt) => {
    console.log("[m2] raw state-change", evt.payload);
  });
  window.__TAURI__.event.listen("display-state", (evt) => {
    const s = evt.payload && evt.payload.state;
    console.log("[m3] DISPLAY", evt.payload);
    if (s && s !== currentState) swapSvg(s);
  });
  window.__TAURI__.event.listen("permission-request", (evt) => {
    console.log("[m2] permission-request", evt.payload);
  });
  console.log("[m2] Tauri event listeners registered");
} else {
  console.warn("[m2] window.__TAURI__ not available");
}
