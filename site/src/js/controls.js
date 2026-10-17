// Schematic, per-circuit knobs and the matching CLI command, all generated from
// the settings schema exported by the Rust core (the same descriptors that
// drive the desktop app and `bender --set`).

const SVG_NS = "http://www.w3.org/2000/svg";
const WIRES = ["--wire-red", "--wire-yellow", "--wire-green", "--wire-white"];
const SLIDER_STEPS = 1000;

export function getPath(obj, id) {
  return id.split(".").reduce((o, k) => o?.[k], obj);
}

export function setPath(obj, id, value) {
  const keys = id.split(".");
  const last = keys.pop();
  const target = keys.reduce((o, k) => (o[k] ??= {}), obj);
  target[last] = value;
}

/** Rounds a slider position's value to the slider's resolution, keeping presets tidy. */
function snap(kind, value) {
  if (kind.log) return Number(value.toPrecision(3));
  const span = kind.max - kind.min;
  const digits = Math.max(0, Math.min(6, 3 - Math.floor(Math.log10(span))));
  return Number(value.toFixed(digits));
}

/** Short label for a value; exact values are kept for presets and commands. */
export function formatValue(setting, value) {
  return setting.kind.type === "float" ? String(Number(value.toPrecision(4))) : String(value);
}

function toSlider(kind, value) {
  if (kind.log) {
    const lo = Math.log(kind.min);
    return Math.round(((Math.log(value) - lo) / (Math.log(kind.max) - lo)) * SLIDER_STEPS);
  }
  return Math.round(((value - kind.min) / (kind.max - kind.min)) * SLIDER_STEPS);
}

function fromSlider(kind, pos) {
  const f = pos / SLIDER_STEPS;
  if (kind.log) {
    const lo = Math.log(kind.min);
    return Math.exp(lo + f * (Math.log(kind.max) - lo));
  }
  return kind.min + f * (kind.max - kind.min);
}

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else if (k === "text") node.textContent = v;
    else node.setAttribute(k, v);
  }
  node.append(...children);
  return node;
}

function svg(tag, attrs = {}) {
  const node = document.createElementNS(SVG_NS, tag);
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
  return node;
}

function jumperGraphic() {
  const g = svg("svg", { viewBox: "0 0 116 48", preserveAspectRatio: "none", "aria-hidden": "true" });
  g.append(
    svg("path", { class: "wire", d: "M18 44 C 22 2, 94 2, 98 44", pathLength: "1" }),
    svg("rect", { class: "socket", x: "12", y: "40", width: "12", height: "8", rx: "1" }),
    svg("rect", { class: "socket", x: "92", y: "40", width: "12", height: "8", rx: "1" }),
    svg("circle", { class: "clip", cx: "18", cy: "42", r: "4" }),
    svg("circle", { class: "clip", cx: "98", cy: "42", r: "4" }),
  );
  const hint = svg("text", { class: "hint", x: "58", y: "38" });
  hint.textContent = "bend";
  g.append(hint);
  return g;
}

/**
 * Builds the schematic and stage panel.
 * `onChange(id, value)` is called for every edit; `onSelect(groupId)` when a
 * circuit is selected; `onMove(groupId, delta)` to move a circuit in the chain.
 */
export function buildControls({ schema, schematic, panel, onChange, onSelect, onMove }) {
  const jumpers = new Map();
  const chips = new Map();
  const circuits = new Map();

  schematic.append(el("li", { class: "terminal in" }, el("span", { text: "In" })));
  schema.groups.forEach((group) => {
    const enabledId = group.settings[0].id;
    const input = el("input", { type: "checkbox", "aria-label": `Bend ${group.label}` });
    input.addEventListener("change", () => onChange(enabledId, input.checked));
    const jumper = el("label", { class: "jumper" }, input, jumperGraphic());
    const ref = el("span", { class: "chip-ref" });

    const body = el(
      "button",
      { type: "button", class: "chip-body", "aria-controls": panel.id },
      ref,
      el("span", { class: "chip-name", text: group.label }),
    );
    body.addEventListener("click", () => onSelect(group.id));

    const li = el("li", { class: "circuit" }, el("div", { class: "chip" }, jumper, body));
    schematic.append(li);
    circuits.set(group.id, { li, ref, jumper });
    jumpers.set(group.id, input);
    chips.set(group.id, body);
  });
  const outTerminal = el("li", { class: "terminal out" }, el("span", { text: "Out" }));
  schematic.append(outTerminal);

  let panelGroup = null;
  let panelInputs = [];

  function renderPanel(group, effect) {
    panelGroup = group.id;
    panelInputs = [];
    const [enabledSetting, ...settings] = group.settings;

    const toggle = el("input", { type: "checkbox", id: "stage-enabled" });
    toggle.addEventListener("change", () => onChange(enabledSetting.id, toggle.checked));
    panelInputs.push({ setting: enabledSetting, sync: (e) => (toggle.checked = getPath(e, enabledSetting.id)) });

    const knobs = el("div", { class: "knobs" });
    for (const setting of settings) {
      knobs.append(buildKnob(setting));
    }

    const earlier = el("button", { type: "button", class: "control quiet", text: "Move earlier" });
    earlier.addEventListener("click", () => onMove(group.id, -1));
    const later = el("button", { type: "button", class: "control quiet", text: "Move later" });
    later.addEventListener("click", () => onMove(group.id, 1));

    panel.replaceChildren(
      el(
        "div",
        { class: "stage-head" },
        el("h3", { text: group.label }),
        el("label", { class: "bend-toggle", for: "stage-enabled" }, toggle, `Bend this circuit`),
        el("p", { text: group.description }),
        el("div", { class: "stage-move" }, earlier, later),
      ),
      knobs,
    );
    sync(effect);
  }

  function buildKnob(setting) {
    const { kind } = setting;
    const domId = `knob-${setting.id.replace(".", "-")}`;
    const help = el("p", { class: "knob-help", id: `${domId}-help`, text: setting.description });

    if (kind.type === "enum") {
      const options = el("div", { class: "segmented" });
      const radios = kind.options.map((opt) => {
        const radio = el("input", { type: "radio", name: domId, value: opt.value });
        radio.addEventListener("change", () => radio.checked && onChange(setting.id, opt.value));
        options.append(el("label", {}, radio, el("span", { text: opt.label })));
        return radio;
      });
      panelInputs.push({
        setting,
        sync: (e) => {
          const v = getPath(e, setting.id);
          for (const r of radios) r.checked = r.value === v;
        },
      });
      return el(
        "fieldset",
        { class: "knob", "aria-describedby": help.id },
        el("legend", { class: "knob-top", text: setting.label }),
        options,
        help,
      );
    }

    if (kind.type === "bool") {
      const box = el("input", { type: "checkbox", id: domId, "aria-describedby": help.id });
      box.addEventListener("change", () => onChange(setting.id, box.checked));
      panelInputs.push({ setting, sync: (e) => (box.checked = getPath(e, setting.id)) });
      return el(
        "div",
        { class: "knob" },
        el("label", { class: "bend-toggle", for: domId }, box, setting.label),
        help,
      );
    }

    const output = el("output", { for: domId });
    const range =
      kind.type === "int"
        ? el("input", { type: "range", id: domId, min: kind.min, max: kind.max, step: 1 })
        : el("input", { type: "range", id: domId, min: 0, max: SLIDER_STEPS, step: 1 });
    range.setAttribute("aria-describedby", help.id);
    range.addEventListener("input", () => {
      const value = kind.type === "int" ? Number(range.value) : snap(kind, fromSlider(kind, Number(range.value)));
      output.textContent = formatValue(setting, value);
      range.setAttribute("aria-valuetext", output.textContent);
      onChange(setting.id, value);
    });
    panelInputs.push({
      setting,
      sync: (e) => {
        const v = getPath(e, setting.id);
        range.value = kind.type === "int" ? v : toSlider(kind, v);
        output.textContent = formatValue(setting, v);
        range.setAttribute("aria-valuetext", output.textContent);
      },
    });
    return el(
      "div",
      { class: "knob" },
      el("label", { class: "knob-top", for: domId }, setting.label, output),
      range,
      help,
    );
  }

  function sync(effect) {
    for (const group of schema.groups) {
      jumpers.get(group.id).checked = Boolean(getPath(effect, group.settings[0].id));
    }
    effect.order.forEach((id, k) => {
      const circuit = circuits.get(id);
      if (!circuit) return;
      schematic.insertBefore(circuit.li, outTerminal);
      circuit.ref.textContent = `U${k + 1}`;
      circuit.jumper.style.setProperty("--wire", `var(${WIRES[k % WIRES.length]})`);
    });
    for (const { sync: s } of panelInputs) s(effect);
    const enabled = panelGroup && getPath(effect, `${panelGroup}.enabled`);
    panel.classList.toggle("is-off", !enabled);
  }

  return {
    select(groupId, effect) {
      for (const [id, chip] of chips) chip.setAttribute("aria-current", String(id === groupId));
      // Centre the chip in the (horizontally scrolling) strip without moving the page.
      const chip = chips.get(groupId).getBoundingClientRect();
      const strip = schematic.getBoundingClientRect();
      schematic.scrollLeft += chip.left + chip.width / 2 - (strip.left + strip.width / 2);
      renderPanel(
        schema.groups.find((g) => g.id === groupId),
        effect,
      );
    },
    sync,
  };
}

/** `bender` invocation reproducing `effect`, listing only non-default settings. */
export function cliCommand(schema, effect) {
  const parts = ["bender -i input.mp4 -o bent.mp4"];
  if (effect.seed) parts.push(`--seed ${effect.seed}`);
  if (effect.order.join(",") !== schema.defaults.order.join(",")) {
    parts.push(`--set order=${effect.order.join(",")}`);
  }
  for (const group of schema.groups) {
    if (!getPath(effect, group.settings[0].id)) continue;
    for (const setting of group.settings) {
      const value = getPath(effect, setting.id);
      if (value !== setting.default) {
        parts.push(`--set ${setting.id}=${value}`);
      }
    }
  }
  return parts.join(" \\\n  ");
}
