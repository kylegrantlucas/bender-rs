import { buildControls, cliCommand, setPath } from "./controls.js";
import { loadEngine } from "./engine.js";
import { CameraSource, FPS, openFile, TestCard } from "./sources.js";

const PRESETS = [
  { file: "rolling-tear", name: "Rolling tear", blurb: "Sync loses lock; the picture rolls and tears over mains hum." },
  { file: "address-shuffle", name: "Address shuffle", blurb: "Crossed address lines read the frame store out of order." },
  { file: "bit-rot", name: "Bit rot", blurb: "Swapped and inverted data bits posterize and recolour." },
  { file: "frozen-store", name: "Frozen store", blurb: "Bands of memory miss their writes and lag behind." },
  { file: "op-amp-howl", name: "Op-amp howl", blurb: "A feedback loop oscillates into drifting rainbow bars." },
  { file: "burst-drift", name: "Burst drift", blurb: "The colour decoder loses phase; hues swim and drop out." },
  { file: "chroma-bleed", name: "Chroma bleed", blurb: "A narrow colour path smears hues right and off their edges." },
  { file: "meltdown", name: "Meltdown", blurb: "Eight circuits bent at once." },
  { file: "sensor-bloom", name: "Sensor bloom", blurb: "A smeared CCD with a hunting auto-exposure." },
  { file: "pocket-camera", name: "Pocket camera", blurb: "A Game Boy Camera retina, dithered to four greens." },
  { file: "worn-tape", name: "Worn tape", blurb: "Head switching, tracking noise and dropouts." },
  { file: "dot-crawl", name: "Dot crawl", blurb: "A real composite signal through a cheap decoder." },
  { file: "datamosh", name: "Datamosh", blurb: "Lost keyframes drag the picture with its own motion." },
  { file: "stride-shear", name: "Stride shear", blurb: "Bent address counters shear and tile the store." },
  { file: "feedback-tunnel", name: "Feedback tunnel", blurb: "A camera pointed at its own monitor, colourized." },
  { file: "scan-lines", name: "Scan lines", blurb: "Rutt/Etra scanlines lifted by brightness." },
];
const FIRST_PRESET = "address-shuffle";

const $ = (id) => document.getElementById(id);
const ui = {
  preview: $("preview"),
  message: $("screen-message"),
  play: $("play"),
  compare: $("compare"),
  seed: $("seed"),
  reseed: $("reseed"),
  lines: $("lines"),
  file: $("file"),
  sourceError: $("source-error"),
  presetList: $("preset-list"),
  clear: $("clear"),
  command: $("command"),
  copy: $("copy-command"),
  download: $("download-preset"),
  copyStatus: $("copy-status"),
};

const work = document.createElement("canvas");
const workCtx = work.getContext("2d", { willReadFrequently: true });
const previewCtx = ui.preview.getContext("2d");

const state = {
  engine: null,
  controls: null,
  effect: null,
  selected: null,
  source: new TestCard(),
  sourceKind: "card",
  playing: true,
  comparing: false,
  clock: 0,
  carry: 0,
  lastTick: null,
  lastKey: null,
  frame: 0,
  dirty: true,
  activePreset: null,
};

function showMessage(text, isError = false) {
  ui.message.textContent = text;
  ui.message.classList.toggle("error", isError);
}

/* ---- effect state ---- */

function commitEffect(next) {
  const error = state.engine.setPreset(next);
  if (error) {
    showMessage(error, true);
    return;
  }
  state.effect = state.engine.getPreset();
  state.controls.sync(state.effect);
  ui.seed.value = state.effect.seed;
  ui.command.textContent = cliCommand(state.engine.schema, state.effect);
  state.dirty = true;
}

function editSetting(id, value) {
  const next = structuredClone(state.effect);
  setPath(next, id, value);
  setActivePreset(null);
  commitEffect(next);
  if (id.endsWith(".enabled") && value) select(id.split(".")[0]);
}

function moveStage(id, delta) {
  const next = structuredClone(state.effect);
  const i = next.order.indexOf(id);
  if (i < 0) return;
  const j = Math.min(Math.max(i + delta, 0), next.order.length - 1);
  [next.order[i], next.order[j]] = [next.order[j], next.order[i]];
  setActivePreset(null);
  commitEffect(next);
}

function select(groupId) {
  if (state.selected === groupId) return;
  state.selected = groupId;
  state.controls.select(groupId, state.effect);
}

function setActivePreset(file) {
  state.activePreset = file;
  for (const button of ui.presetList.querySelectorAll("button")) {
    button.setAttribute("aria-pressed", String(button.dataset.file === file));
  }
}

async function loadPreset(file) {
  try {
    const response = await fetch(`presets/${file}.json`);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const preset = await response.json();
    state.engine.reset();
    commitEffect(preset);
    setActivePreset(file);
    const firstOn = state.effect.order.find((id) => state.effect[id]?.enabled);
    if (firstOn) select(firstOn);
  } catch (err) {
    showMessage(`Preset ${file} failed to load: ${err.message}`, true);
  }
}

/* ---- rendering ---- */

function processingSize() {
  const lines = Number(ui.lines.value);
  const width = Math.max(2, 2 * Math.round((lines * state.source.aspect()) / 2));
  return [width, lines];
}

function render(frame) {
  const [w, h] = processingSize();
  if (work.width !== w || work.height !== h) {
    work.width = w;
    work.height = h;
  }
  if (ui.preview.width !== w || ui.preview.height !== h) {
    ui.preview.width = w;
    ui.preview.height = h;
    ui.preview.parentElement.style.setProperty("--aspect", `${w} / ${h}`);
  }
  state.source.draw(workCtx, w, h, state.clock);
  if (state.comparing) {
    previewCtx.drawImage(work, 0, 0);
    return;
  }
  const pixels = workCtx.getImageData(0, 0, w, h);
  previewCtx.putImageData(state.engine.process(pixels.data, w, h, frame), 0, 0);
}

// The effect's temporal bends (sensor, ISP loops, datamosh, memory hold,
// feedback) need consecutive frame numbers, so
// the effect frame advances by exactly one per new source picture, never by
// wall-clock jumps; a slow device slows the test card instead of skipping.
function tick(now) {
  requestAnimationFrame(tick);
  if (state.lastTick !== null && state.playing) {
    const step = 1000 / FPS;
    state.carry += now - state.lastTick;
    if (state.carry >= step) {
      state.clock += 1;
      state.carry = Math.min(state.carry - step, step);
    }
  }
  state.lastTick = now;
  if (!state.engine || !state.source.ready()) return;
  const key = state.source.frameKey(state.clock);
  if (key !== state.lastKey) {
    state.lastKey = key;
    state.frame += 1;
  } else if (!state.dirty) {
    return;
  }
  state.dirty = false;
  render(state.frame);
}

/* ---- sources ---- */

function setSourceError(text) {
  ui.sourceError.textContent = text;
  ui.sourceError.hidden = !text;
}

function checkSourceRadio(kind) {
  document.querySelector(`input[name="source"][value="${kind}"]`).checked = true;
}

function useSource(source, kind) {
  state.source.stop();
  state.source = source;
  state.sourceKind = kind;
  checkSourceRadio(kind);
  state.engine?.reset();
  if (state.playing) source.play();
  else source.pause();
  setSourceError("");
  state.dirty = true;
}

async function chooseSource(kind) {
  if (kind === "card") {
    useSource(new TestCard(), "card");
  } else if (kind === "camera") {
    try {
      useSource(await CameraSource.open(), "camera");
    } catch (err) {
      checkSourceRadio(state.sourceKind);
      setSourceError(`Camera unavailable: ${err.message}`);
    }
  } else if (kind === "file") {
    ui.file.click();
  }
}

/* ---- wiring ---- */

function wireEvents() {
  for (const radio of document.querySelectorAll('input[name="source"]')) {
    radio.addEventListener("change", () => radio.checked && chooseSource(radio.value));
  }
  // Picking "file" again while it is selected opens another file.
  document.querySelector('input[name="source"][value="file"]').addEventListener("click", () => {
    if (state.sourceKind === "file") ui.file.click();
  });
  ui.file.addEventListener("change", async () => {
    const file = ui.file.files[0];
    ui.file.value = "";
    if (!file) return;
    try {
      useSource(await openFile(file), "file");
    } catch (err) {
      checkSourceRadio(state.sourceKind);
      setSourceError(err.message);
    }
  });
  ui.file.addEventListener("cancel", () => checkSourceRadio(state.sourceKind));

  ui.play.addEventListener("click", () => {
    state.playing = !state.playing;
    ui.play.textContent = state.playing ? "Pause" : "Play";
    if (state.playing) state.source.play();
    else state.source.pause();
  });

  const startCompare = () => {
    state.comparing = true;
    state.dirty = true;
  };
  const endCompare = () => {
    if (!state.comparing) return;
    state.comparing = false;
    state.dirty = true;
  };
  ui.compare.addEventListener("pointerdown", startCompare);
  ui.compare.addEventListener("pointerup", endCompare);
  ui.compare.addEventListener("pointerleave", endCompare);
  ui.compare.addEventListener("pointercancel", endCompare);
  ui.compare.addEventListener("keydown", (e) => {
    if ((e.key === " " || e.key === "Enter") && !e.repeat) {
      e.preventDefault();
      startCompare();
    }
  });
  ui.compare.addEventListener("keyup", (e) => {
    if (e.key === " " || e.key === "Enter") endCompare();
  });
  ui.compare.addEventListener("blur", endCompare);

  ui.seed.addEventListener("change", () => {
    const seed = Math.trunc(Number(ui.seed.value));
    editSetting("seed", Number.isFinite(seed) ? Math.max(-2147483648, Math.min(2147483647, seed)) : 0);
  });
  ui.reseed.addEventListener("click", () => editSetting("seed", Math.floor(Math.random() * 100000)));
  ui.lines.addEventListener("change", () => {
    state.engine.reset();
    state.dirty = true;
  });

  ui.clear.addEventListener("click", () => {
    const next = structuredClone(state.effect);
    for (const group of state.engine.schema.groups) next[group.id].enabled = false;
    setActivePreset(null);
    commitEffect(next);
  });

  ui.copy.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(ui.command.textContent);
      ui.copyStatus.textContent = "Command copied";
    } catch {
      ui.copyStatus.textContent = "Copying is blocked here; select the command and copy it manually";
    }
  });

  ui.download.addEventListener("click", () => {
    const blob = new Blob([`${JSON.stringify(state.engine.getPreset(), null, 2)}\n`], { type: "application/json" });
    const link = document.createElement("a");
    link.href = URL.createObjectURL(blob);
    link.download = `${state.activePreset ?? "bend"}.json`;
    link.click();
    URL.revokeObjectURL(link.href);
  });
}

function buildPresetList() {
  for (const preset of PRESETS) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "preset";
    button.dataset.file = preset.file;
    button.setAttribute("aria-pressed", "false");
    const name = document.createElement("span");
    name.className = "preset-name";
    name.textContent = preset.name;
    const blurb = document.createElement("span");
    blurb.className = "preset-blurb";
    blurb.textContent = preset.blurb;
    button.append(name, blurb);
    button.addEventListener("click", () => loadPreset(preset.file));
    const item = document.createElement("li");
    item.append(button);
    ui.presetList.append(item);
  }
}

async function main() {
  buildPresetList();
  wireEvents();
  requestAnimationFrame(tick);
  try {
    state.engine = await loadEngine("bender.wasm");
  } catch (err) {
    showMessage(`The effect engine failed to load: ${err.message}`, true);
    return;
  }
  state.controls = buildControls({
    schema: state.engine.schema,
    schematic: $("schematic"),
    panel: $("stage-panel"),
    onChange: editSetting,
    onSelect: select,
    onMove: moveStage,
  });
  state.effect = state.engine.getPreset();
  select(state.engine.schema.groups[0].id);
  showMessage("");
  commitEffect(state.effect);
  await loadPreset(FIRST_PRESET);
}

main();
