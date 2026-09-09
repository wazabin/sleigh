import init, { compile, decode, highlight, presets } from "./pkg/sleigh_web.js";

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
});

// ── Highlighting ──────────────────────────────────────────────────────────
// The crate's tolerant lexer returns byte ranges; CodeMirror wants character
// positions. Marks are replaced wholesale after each edit, debounced.
let marks = [];
let highlightTimer = null;
function byteToCharIndex(text) {
  // Identity for ASCII, which is nearly every spec.
  if (!/[^\x00-\x7f]/.test(text)) return (b) => b;
  const map = [];
  let byte = 0;
  for (let i = 0; i < text.length; i++) {
    map[byte] = i;
    const code = text.codePointAt(i);
    const len = code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4;
    byte += len;
    if (code >= 0x10000) i++;
  }
  map[byte] = text.length;
  return (b) => map[b] ?? text.length;
}
function rehighlight() {
  const text = editor.getValue();
  const toChar = byteToCharIndex(text);
  editor.operation(() => {
    for (const m of marks) m.clear();
    marks = [];
    for (const [start, end, kind] of JSON.parse(highlight(text))) {
      marks.push(editor.markText(editor.posFromIndex(toChar(start)), editor.posFromIndex(toChar(end)), { className: "tok-" + kind }));
    }
  });
}
editor.on("change", () => {
  clearTimeout(highlightTimer);
  highlightTimer = setTimeout(rehighlight, 80);
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
    $("preset").value = state.preset || "";
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

// ── Presets ───────────────────────────────────────────────────────────────
// Each preset is a small toy ISA shipped as editable source. Picking one
// loads its spec, bytes and address; editing afterwards is just editing.
let PRESETS = [];
function loadPreset(name) {
  const preset = PRESETS.find((p) => p.name === name) || PRESETS[0];
  if (!preset) return;
  editor.setValue(preset.source);
  $("bytes").value = preset.bytes;
  $("address").value = preset.address;
}
$("preset").onchange = () => {
  loadPreset($("preset").value);
  runCompile();
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
  if (!ready) return;
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
  if (!lastCompile || !lastCompile.ok) {
    showListing({ error: "compile the spec first" });
    return;
  }
  const output = JSON.parse(decode(JSON.stringify(opts)));
  showListing(output);
  $("json").textContent = JSON.stringify(output, null, 2);
  if (output.error) {
    status.className = "status error";
    status.textContent = output.error;
    return;
  }
  status.className = "status ok";
  status.textContent =
    `ok: ${lastCompile.registers} registers, ${lastCompile.tables} tables` +
    ` · decoded ${output.instructions.length} instruction(s)`;
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
init().then(() => {
  ready = true;
  PRESETS = JSON.parse(presets());
  const select = $("preset");
  for (const p of PRESETS) {
    const option = document.createElement("option");
    option.value = p.name;
    option.textContent = p.name;
    select.append(option);
  }
  if (!loadHash() || !editor.getValue().trim()) {
    // Nothing shared: open on the first toy.
    select.value = PRESETS[0].name;
    loadPreset(PRESETS[0].name);
  }
  runCompile();
}).catch((e) => {
  status.className = "status error";
  status.textContent = "failed to load wasm: " + e;
});
