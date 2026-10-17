// Thin wrapper over the bender-wasm C ABI (see crates/bender-wasm/src/lib.rs).

const encoder = new TextEncoder();
const decoder = new TextDecoder();

async function instantiate(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`${url} returned HTTP ${response.status}`);
  }
  if (WebAssembly.instantiateStreaming) {
    try {
      return (await WebAssembly.instantiateStreaming(response.clone(), {})).instance;
    } catch {
      // Served without the application/wasm MIME type; compile from bytes instead.
    }
  }
  return (await WebAssembly.instantiate(await response.arrayBuffer(), {})).instance;
}

export async function loadEngine(url) {
  const x = (await instantiate(url)).exports;
  const bytes = (ptr, len) => new Uint8Array(x.memory.buffer, ptr, len);
  const readText = (len) => decoder.decode(bytes(x.bender_text_ptr(), len));
  const writeText = (text) => {
    const data = encoder.encode(text);
    bytes(x.bender_text_buffer(data.length), data.length).set(data);
    return data.length;
  };

  return {
    schema: JSON.parse(readText(x.bender_schema())),

    /** Loads a preset object; returns null on success or the error message. */
    setPreset(preset) {
      const status = x.bender_set_preset(writeText(JSON.stringify(preset)));
      return status < 0 ? readText(-status) : null;
    },

    /** The current preset after range clamping. */
    getPreset() {
      return JSON.parse(readText(x.bender_get_preset()));
    },

    /** Renders RGBA pixels and returns an ImageData view of the result. */
    process(rgba, width, height, frame) {
      const len = width * height * 4;
      bytes(x.bender_frame_buffer(width, height), len).set(rgba);
      if (x.bender_process(width, height, frame) !== 0) {
        throw new Error("frame buffer size mismatch");
      }
      const out = new Uint8ClampedArray(x.memory.buffer, x.bender_output(), len);
      return new ImageData(out, width, height);
    },

    /** Forgets the emulated frame store (call when the source changes). */
    reset() {
      x.bender_reset();
    },
  };
}
