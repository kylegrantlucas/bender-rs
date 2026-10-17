// Picture sources. Each draws its current picture into a canvas context of the
// processing size, reports its aspect ratio, and returns a frame key that
// changes whenever a new picture is available.

export const FPS = 30;

const BARS = ["#c0c0c0", "#c0c000", "#00c0c0", "#00c000", "#c000c0", "#c00000", "#0000c0"];
const CASTELLATIONS = ["#0000c0", "#131313", "#c000c0", "#131313", "#00c0c0", "#131313", "#c0c0c0"];

/** Animated colour-bar test card drawn procedurally; time comes from the frame clock. */
export class TestCard {
  aspect() {
    return 4 / 3;
  }

  ready() {
    return true;
  }

  frameKey(clock) {
    return clock;
  }

  draw(ctx, w, h, clock) {
    const t = clock / FPS;
    const barW = w / BARS.length;
    const top = Math.round(h * 0.62);
    const mid = Math.round(h * 0.7);

    BARS.forEach((c, i) => {
      ctx.fillStyle = c;
      ctx.fillRect(Math.floor(i * barW), 0, Math.ceil(barW), top);
    });
    CASTELLATIONS.forEach((c, i) => {
      ctx.fillStyle = c;
      ctx.fillRect(Math.floor(i * barW), top, Math.ceil(barW), mid - top);
    });

    // luma ramp and black/white patches
    const ramp = ctx.createLinearGradient(0, 0, w * 0.6, 0);
    ramp.addColorStop(0, "#000");
    ramp.addColorStop(1, "#fff");
    ctx.fillStyle = ramp;
    ctx.fillRect(0, mid, w * 0.6, h - mid);
    ctx.fillStyle = "#fff";
    ctx.fillRect(w * 0.6, mid, w * 0.2, h - mid);
    ctx.fillStyle = "#101010";
    ctx.fillRect(w * 0.8, mid, w * 0.2, h - mid);

    // ring and crosshair
    const r = h * 0.36;
    ctx.lineWidth = Math.max(2, h / 160);
    ctx.strokeStyle = "#fff";
    ctx.beginPath();
    ctx.arc(w / 2, h / 2, r, 0, Math.PI * 2);
    ctx.moveTo(w / 2 - r * 0.25, h / 2);
    ctx.lineTo(w / 2 + r * 0.25, h / 2);
    ctx.moveTo(w / 2, h / 2 - r * 0.25);
    ctx.lineTo(w / 2, h / 2 + r * 0.25);
    ctx.stroke();

    // a disc sweeping across the ramp so time-based bends have motion to act on
    const x = w * (0.5 + 0.42 * Math.sin(t * 1.3));
    ctx.fillStyle = "#ff3d2e";
    ctx.beginPath();
    ctx.arc(x, mid + (h - mid) / 2, (h - mid) * 0.36, 0, Math.PI * 2);
    ctx.fill();

    // timecode box
    const total = Math.floor(clock);
    const pad = (n) => String(n).padStart(2, "0");
    const tc = `${pad(Math.floor(total / (FPS * 60)) % 60)}:${pad(Math.floor(total / FPS) % 60)}:${pad(total % FPS)}`;
    const fontSize = Math.round(h / 14);
    ctx.font = `700 ${fontSize}px "B612 Mono", ui-monospace, monospace`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    const boxW = fontSize * 6.5;
    ctx.fillStyle = "#000";
    ctx.fillRect(w / 2 - boxW / 2, h * 0.08, boxW, fontSize * 1.5);
    ctx.fillStyle = "#fff";
    ctx.fillText(tc, w / 2, h * 0.08 + fontSize * 0.78);
  }

  play() {}
  pause() {}
  stop() {}
}

/** A <video> element fed by a camera stream or a user file. */
class VideoSource {
  constructor(video) {
    this.video = video;
  }

  aspect() {
    const v = this.video;
    return v.videoWidth && v.videoHeight ? v.videoWidth / v.videoHeight : 4 / 3;
  }

  ready() {
    return this.video.readyState >= 2;
  }

  draw(ctx, w, h) {
    ctx.drawImage(this.video, 0, 0, w, h);
  }

  play() {
    return this.video.play().catch(() => {});
  }

  pause() {
    this.video.pause();
  }
}

function makeVideo() {
  const video = document.createElement("video");
  video.muted = true;
  video.playsInline = true;
  video.loop = true;
  return video;
}

export class CameraSource extends VideoSource {
  static async open() {
    if (!navigator.mediaDevices?.getUserMedia) {
      throw new Error("This browser does not offer camera access on this page.");
    }
    const stream = await navigator.mediaDevices.getUserMedia({
      video: { width: { ideal: 1280 }, height: { ideal: 720 } },
      audio: false,
    });
    const video = makeVideo();
    video.srcObject = stream;
    await video.play();
    return new CameraSource(video);
  }

  frameKey(clock) {
    return clock;
  }

  stop() {
    this.video.srcObject?.getTracks().forEach((track) => track.stop());
    this.video.srcObject = null;
  }
}

export class VideoFileSource extends VideoSource {
  static async open(file) {
    const video = makeVideo();
    const url = URL.createObjectURL(file);
    video.src = url;
    try {
      await new Promise((resolve, reject) => {
        video.addEventListener("loadeddata", resolve, { once: true });
        video.addEventListener("error", () => reject(new Error(`${file.name} could not be decoded by this browser.`)), {
          once: true,
        });
      });
    } catch (err) {
      URL.revokeObjectURL(url);
      throw err;
    }
    const source = new VideoFileSource(video);
    source.url = url;
    await source.play();
    return source;
  }

  frameKey() {
    return Math.floor(this.video.currentTime * FPS);
  }

  stop() {
    this.video.pause();
    this.video.removeAttribute("src");
    this.video.load();
    URL.revokeObjectURL(this.url);
  }
}

export class ImageFileSource {
  static async open(file) {
    try {
      return new ImageFileSource(await createImageBitmap(file));
    } catch {
      throw new Error(`${file.name} could not be decoded by this browser.`);
    }
  }

  constructor(bitmap) {
    this.bitmap = bitmap;
  }

  aspect() {
    return this.bitmap.width / this.bitmap.height;
  }

  ready() {
    return true;
  }

  frameKey(clock) {
    return clock;
  }

  draw(ctx, w, h) {
    ctx.drawImage(this.bitmap, 0, 0, w, h);
  }

  play() {}
  pause() {}

  stop() {
    this.bitmap.close();
  }
}

export async function openFile(file) {
  if (file.type.startsWith("video/")) return VideoFileSource.open(file);
  if (file.type.startsWith("image/")) return ImageFileSource.open(file);
  throw new Error(`${file.name} is not an image or video.`);
}
