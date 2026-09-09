import init, { compile, decode, decode_preset, presets, preset_stats, sample } from "./pkg/sleigh_web.js";

const $ = (id) => document.getElementById(id);
const status = $("status");

// ── Theme ──────────────────────────────────────────────────────────────────
function applyTheme(theme) {
  if (theme) document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme;
  try { theme ? localStorage.setItem("theme", theme) : localStorage.removeItem("theme"); } catch {}
}
function toggleTheme() {
  const dark = matchMedia("(prefers-color-scheme: dark)").matches;
  const current = document.documentElement.dataset.theme || (dark ? "dark" : "light");
  applyTheme(current === "dark" ? "light" : "dark");
}
try { applyTheme(localStorage.getItem("theme") || null); } catch {}
$("theme").onclick = toggleTheme;

// ── Editor ─────────────────────────────────────────────────────────────────
const editor = CodeMirror.fromTextArea($("source"), {
  lineNumbers: true,
  lineWrapping: false,
  mode: null,
  readOnly: false,
});

// ── Split divider ──────────────────────────────────────────────────────────
{
  const divider = $("divider");
  const input = document.querySelector(".pane.input");
  try {
    const saved = localStorage.getItem("split");
    if (saved) input.style.flexBasis = saved;
  } catch {}
  divider.onpointerdown = (e) => {
    divider.setPointerCapture(e.pointerId);
    const vertical = matchMedia("(max-width: 800px)").matches;
    const move = (e) => {
      const rect = divider.parentElement.getBoundingClientRect();
      const frac = vertical ? (e.clientY - rect.top) / rect.height : (e.clientX - rect.left) / rect.width;
      const pct = Math.min(85, Math.max(15, frac * 100)).toFixed(1) + "%";
      input.style.flexBasis = pct;
      try { localStorage.setItem("split", pct); } catch {}
      editor.refresh();
    };
    divider.onpointermove = move;
    divider.onpointerup = () => { divider.onpointermove = null; };
  };
}

// ── Tabs ───────────────────────────────────────────────────────────────────
for (const tab of document.querySelectorAll('[role="tab"]')) {
  tab.onclick = () => selectTab(tab.dataset.tab);
}
function selectTab(name) {
  for (const tab of document.querySelectorAll('[role="tab"]')) {
    tab.setAttribute("aria-selected", String(tab.dataset.tab === name));
  }
  for (const panel of document.querySelectorAll(".pane.result [data-tab]:not([role])")) {
    panel.hidden = panel.dataset.tab !== name;
  }
  try { localStorage.setItem("tab", name); } catch {}
}
try { selectTab(localStorage.getItem("tab") || "listing"); } catch {}

// ── Options ↔ URL hash ─────────────────────────────────────────────────────
function saveHash() {
  const state = {
    source: editor.getValue(),
    preset: $("preset").value,
    lint: $("lint").checked,
    bytes: $("bytes").value,
    address: $("address").value,
    pcode: $("pcode").checked,
  };
  const encoded = encodeURIComponent(JSON.stringify(state));
  if (encoded.length < 8192) history.replaceState(null, "", "#" + encoded);
}
function loadHash() {
  if (!location.hash) return false;
  try {
    const state = JSON.parse(decodeURIComponent(location.hash.slice(1)));
    $("preset").value = state.preset || "custom";
    editor.setValue(state.source || "");
    $("lint").checked = !!state.lint;
    $("bytes").value = state.bytes || "";
    $("address").value = state.address || "0x0";
    $("pcode").checked = !!state.pcode;
    return true;
  } catch {
    return false;
  }
}

// ── Preset mode ──────────────────────────────────────────────────────────
function isPreset() {
  return $("preset").value !== "custom";
}
// Something to decode as soon as a preset is chosen, so the listing is
// never empty on arrival.
const DEFAULT_BYTES = {
  custom: "16203b4450fa00",
  x64: "4889d84801c8c3",
  x86: "89d801c8c3",
  aarch64: "000080d2c0035fd6",
  riscv: "1305100067800000",
};
function fillDefaultBytes(force) {
  const input = $("bytes");
  if (force || !input.value.trim()) input.value = DEFAULT_BYTES[$("preset").value] || "";
}
function updatePresetMode() {
  const note = $("preset-note");
  if (isPreset()) {
    editor.setOption("readOnly", true);
    note.hidden = false;
    note.textContent =
      `${$("preset").value} is a precompiled specification embedded in the wasm bundle; ` +
      `its .slaspec source is a tree of #include files and cannot be shown here. ` +
      `Switch the preset to "custom" to edit a self-contained spec instead.`;
  } else {
    editor.setOption("readOnly", false);
    note.hidden = true;
  }
}
$("preset").onchange = () => {
  updatePresetMode();
  fillDefaultBytes(true);
  if (isPreset()) {
    showSpecStats(JSON.parse(preset_stats($("preset").value)));
    runDecode();
  } else {
    editor.setValue(sample_source());
    runCompile();
  }
  saveHash();
};

// ── Diagnostics / markers ───────────────────────────────────────────────
function clearMarks() {
  editor.eachLine((line) => editor.removeLineClass(line, "background"));
}
function showDiagnostics(diagnostics) {
  const box = $("diagnostics");
  box.replaceChildren();
  clearMarks();
  for (const d of diagnostics || []) {
    const sev = (d.severity || "").toLowerCase();
    const div = document.createElement("div");
    div.className = sev.startsWith("warn") ? "warn" : sev.startsWith("err") ? "error" : "";
    div.textContent = `${d.severity}: ${d.message} (${d.line}:${d.column})`;
    box.append(div);
    if (d.line > 0) {
      const cls = sev.startsWith("warn") ? "cm-warn-line" : "cm-error-line";
      editor.addLineClass(d.line - 1, "background", cls);
    }
  }
}

// ── Spec tab ───────────────────────────────────────────────────────────
function showSpecStats(output) {
  const spec = $("spec");
  if (!output || output.error || !output.ok) {
    spec.textContent = output && output.error ? output.error : "not compiled";
    return;
  }
  const spaces = (output.spaces || [])
    .map((s) => `<dt>${s.name ?? "<unnamed>"}</dt><dd>size=${s.size} wordsize=${s.wordsize}</dd>`)
    .join("");
  spec.innerHTML = `
    <h3>Summary</h3>
    <dl>
      <dt>registers</dt><dd>${output.registers}</dd>
      <dt>tables</dt><dd>${output.tables}</dd>
      <dt>context fields</dt><dd>${output.context_fields}</dd>
      <dt>default space</dt><dd>${output.default_space ?? "<none>"}</dd>
      <dt>compile time</dt><dd>${output.compile_ms.toFixed(2)} ms</dd>
    </dl>
    <h3>Spaces</h3>
    <dl>${spaces}</dl>
  `;
}

// ── Listing tab ────────────────────────────────────────────────────────
function showListing(output) {
  const listing = $("listing");
  if (!output || output.error) {
    listing.textContent = output ? output.error : "";
    return;
  }
  const lines = [];
  for (const insn of output.instructions) {
    lines.push(`<span class="insn"><span class="addr">${insn.address}</span>  <span class="bytes">${insn.bytes.padEnd(24)}</span>  ${escapeHtml(insn.text)}</span>`);
    for (const p of insn.pcode || []) {
      lines.push(`<span class="pcode-line">${escapeHtml(p)}</span>`);
    }
  }
  listing.innerHTML = lines.join("\n") || "(no instructions)";
}
function escapeHtml(s) {
  return s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));
}

// ── Running ────────────────────────────────────────────────────────────
let ready = false;
let lastCompile = null;
let decodeTimer = null;

function runCompile() {
  if (!ready || isPreset()) return;
  const opts = {
    source: editor.getValue(),
    defines: {},
    context: {},
    lint: $("lint").checked,
  };
  const t0 = performance.now();
  const output = JSON.parse(compile(JSON.stringify(opts)));
  const t1 = performance.now();
  $("time").textContent = (t1 - t0).toFixed(1) + " ms";
  lastCompile = output;
  showSpecStats(output);
  $("json").textContent = JSON.stringify(output, null, 2);
  if (output.error) {
    status.className = "status error";
    status.textContent = output.error;
    showDiagnostics([]);
    return;
  }
  showDiagnostics(output.diagnostics);
  if (!output.ok) {
    status.className = "status error";
    const first = output.diagnostics.find((d) => d.severity === "Error") || output.diagnostics[0];
    status.textContent = first ? `${first.severity}: ${first.message}` : "compile failed";
    return;
  }
  status.className = "status ok";
  status.textContent =
    `ok: ${output.registers} registers, ${output.tables} tables, ${output.context_fields} context fields` +
    (output.diagnostics.length ? `, ${output.diagnostics.length} lint finding(s)` : "") +
    ` · ${output.compile_ms.toFixed(2)} ms`;
  saveHash();
  runDecode();
}

function runDecode() {
  if (!ready) return;
  const bytes = $("bytes").value.trim();
  if (!bytes) {
    showListing(null);
    $("json").textContent = lastCompile ? JSON.stringify(lastCompile, null, 2) : "";
    return;
  }
  const opts = {
    bytes,
    address: $("address").value.trim() || "0x0",
    pcode: $("pcode").checked,
  };
  let output;
  if (isPreset()) {
    output = JSON.parse(decode_preset(JSON.stringify({ ...opts, arch: $("preset").value })));
  } else {
    if (!lastCompile || !lastCompile.ok) {
      showListing({ error: "compile the spec first" });
      return;
    }
    output = JSON.parse(decode(JSON.stringify(opts)));
  }
  showListing(output);
  if (!isPreset()) {
    $("json").textContent = JSON.stringify(output, null, 2);
  }
  if (output.error) {
    status.className = "status error";
    status.textContent = output.error;
    return;
  }
  if (!isPreset()) {
    status.className = "status ok";
    status.textContent =
      `ok: ${lastCompile.registers} registers, ${lastCompile.tables} tables` +
      ` · decoded ${output.instructions.length} instruction(s)`;
  } else {
    status.className = "status ok";
    status.textContent = `decoded ${output.instructions.length} instruction(s) with ${output.arch}`;
  }
  saveHash();
}

function scheduleDecode() {
  clearTimeout(decodeTimer);
  decodeTimer = setTimeout(runDecode, 200);
}

$("compile").onclick = runCompile;
for (const id of ["bytes", "address", "pcode"]) {
  $(id).addEventListener("input", scheduleDecode);
}
document.addEventListener("keydown", (e) => {
  if (e.ctrlKey && e.key === "Enter") { e.preventDefault(); runCompile(); }
  if (e.ctrlKey && e.key === "/") { e.preventDefault(); toggleTheme(); }
  if (e.key === "Escape") document.activeElement?.blur();
});
new ResizeObserver(() => editor.refresh()).observe(document.querySelector(".pane.input"));

// ── Boot ───────────────────────────────────────────────────────────────
let cachedSample = null;
function sample_source() {
  if (cachedSample === null) cachedSample = sample();
  return cachedSample;
}

init().then(() => {
  ready = true;
  if (!loadHash()) {
    editor.setValue(sample());
  }
  // A shared link may carry a preset and bytes but no source; custom mode
  // still needs a spec to compile.
  if (!isPreset() && !editor.getValue().trim()) editor.setValue(sample());
  updatePresetMode();
  fillDefaultBytes(false);
  if (isPreset()) {
    showSpecStats(JSON.parse(preset_stats($("preset").value)));
    runDecode();
  } else {
    runCompile();
  }
}).catch((e) => {
  status.className = "status error";
  status.textContent = "failed to load wasm: " + e;
});
