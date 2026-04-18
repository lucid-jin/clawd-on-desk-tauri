// M1/M3/M7 renderer: SVG render + eye tracking + state-driven SVG swap +
// drag/click interaction.

const clawdEl = document.getElementById("clawd");
const container = document.getElementById("pet-container");

const MAX_OFFSET = 3;
const BODY_SCALE = 0.33;
const SHADOW_STRETCH = 0.15;
const SHADOW_SHIFT = 0.3;
const BASE = "assets/svg/";
const DRAG_THRESHOLD = 3;
const MULTI_CLICK_MS = 500;
const REACTION_MS = 2500;

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
  // reaction overlays (M7)
  "react-left": "clawd-react-left.svg",
  "react-right": "clawd-react-right.svg",
  "react-double": "clawd-react-double.svg",
  "react-drag": "clawd-react-drag.svg",
};

const EYE_TRACKING_STATES = new Set(["idle", "mini-idle"]);

const MINI_STATE_MAP = {
  idle: "mini-idle",
  thinking: "mini-idle",
  typing: "mini-idle",
  working: "mini-idle",
  juggling: "mini-idle",
  conducting: "mini-idle",
  building: "mini-idle",
  carrying: "mini-idle",
  sweeping: "mini-idle",
  attention: "mini-happy",
  happy: "mini-happy",
  notification: "mini-alert",
  error: "mini-alert",
  sleeping: "mini-sleep",
};

let miniActive = false;

const MINI_STATE_SVG = {
  "mini-idle": "clawd-mini-idle.svg",
  "mini-alert": "clawd-mini-alert.svg",
  "mini-happy": "clawd-mini-happy.svg",
  "mini-sleep": "clawd-mini-sleep.svg",
  "mini-peek": "clawd-mini-peek.svg",
};

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
  const resolved = miniActive ? (MINI_STATE_MAP[state] || "mini-idle") : state;
  const file = (miniActive ? MINI_STATE_SVG[resolved] : STATE_SVG[resolved]);
  if (!file) {
    console.warn("[m3] no SVG mapping for state:", state, "mini:", miniActive);
    return;
  }
  currentState = resolved;
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
    if (s) lastDisplayState = s;
    // Don't overwrite an active reaction
    if (reactionActive) return;
    if (s) swapSvg(s);
  });
  window.__TAURI__.event.listen("permission-request", (evt) => {
    console.log("[m2] permission-request", evt.payload);
  });
  window.__TAURI__.event.listen("mini-state", (evt) => {
    miniActive = !!evt.payload;
    console.log("[m8] mini-state =", miniActive);
    swapSvg(lastDisplayState || "idle");
  });
  console.log("[m2] Tauri event listeners registered");
} else {
  console.warn("[m2] window.__TAURI__ not available");
}

// ── M7: drag + click reaction ──────────────────────────────────────────────
let dragStart = null;
let dragging = false;
let clickTimes = [];
let reactionActive = false;
let reactionTimer = null;
let lastDisplayState = "idle";

function currentWindow() {
  // Tauri v2 JS API path — prefer webviewWindow, fall back to window
  const t = window.__TAURI__;
  if (!t) return null;
  if (t.webviewWindow && t.webviewWindow.getCurrentWebviewWindow) {
    return t.webviewWindow.getCurrentWebviewWindow();
  }
  if (t.window && t.window.getCurrentWindow) {
    return t.window.getCurrentWindow();
  }
  return null;
}

function playReaction(stateName) {
  const file = STATE_SVG[stateName];
  if (!file) return;
  reactionActive = true;
  if (reactionTimer) clearTimeout(reactionTimer);
  currentState = stateName;
  clawdEl.setAttribute("data", BASE + file);
  reactionTimer = setTimeout(() => {
    reactionActive = false;
    swapSvg(lastDisplayState || "idle");
  }, REACTION_MS);
}

container.addEventListener("pointerdown", (ev) => {
  if (ev.button !== 0) return;
  dragStart = { x: ev.clientX, y: ev.clientY, t: Date.now() };
  dragging = false;
});

container.addEventListener("pointermove", async (ev) => {
  if (!dragStart || dragging) return;
  const dx = ev.clientX - dragStart.x;
  const dy = ev.clientY - dragStart.y;
  if (Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
  dragging = true;
  const win = currentWindow();
  if (win) {
    try {
      playReaction("react-drag");
      await win.startDragging();
    } catch (err) {
      console.warn("[m7] startDragging failed", err);
    }
  }
});

container.addEventListener("pointerup", async (ev) => {
  const wasDrag = dragging;
  dragStart = null;
  dragging = false;
  if (wasDrag) {
    // After drag release, check edge snap
    try {
      const snapped = await window.__TAURI__.core.invoke("maybe_snap_right_cmd");
      if (snapped) console.log("[m8] snapped to right edge");
    } catch (err) {
      console.warn("[m8] snap check failed", err);
    }
    return;
  }

  // Treat as click — record for multi-click detection
  const now = Date.now();
  clickTimes = clickTimes.filter((t) => now - t < MULTI_CLICK_MS);
  clickTimes.push(now);

  if (clickTimes.length >= 4) {
    playReaction("react-double");
    clickTimes = [];
  } else if (clickTimes.length === 2) {
    const side = ev.clientX > container.clientWidth / 2 ? "react-right" : "react-left";
    playReaction(side);
  }
});

window.addEventListener("contextmenu", (ev) => {
  // Right-click — M7 MVP: no custom menu yet. Prevent default system menu.
  ev.preventDefault();
});
