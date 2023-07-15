# Training + Exporting YOLO → TFLite (docs/TRAINING_EXPORT_TFLITE.md)

This guide trains a YOLO detector and exports a **TFLite** model suitable for Raspberry Pi CPU and optionally **Coral EdgeTPU**.

> Use only where you have permission. Avoid recording neighbors/public areas.

---

## 1) Train on GPU (Ultralytics YOLO)

```bash
python3 -m venv .venv
. .venv/bin/activate
pip install ultralytics
```

Example data.yaml:

path: /data/myset
train: images/train
val: images/val
names:
  0: btr

Train (nano is best for Pi):

<pre class="overflow-visible! px-0!" data-start="2753" data-end="2843"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"></div></pre>

<pre class="overflow-visible! px-0!" data-start="2753" data-end="2843"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"><div class="overflow-y-auto p-4" dir="ltr"><code class="whitespace-pre! language-bash"><span><span>yolo detect train model=yolov8n.pt data=data.yaml imgsz=640 epochs=80 batch=16
</span></span></code></div></div></pre>

---

## 2) Export to TFLite (CPU)

<pre class="overflow-visible! px-0!" data-start="2879" data-end="2966"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"></div></pre>

<pre class="overflow-visible! px-0!" data-start="2879" data-end="2966"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"><div class="overflow-y-auto p-4" dir="ltr"><code class="whitespace-pre! language-bash"><span><span>yolo </span><span>export</span><span> model=runs/detect/train/weights/best.pt format=tflite imgsz=640
</span></span></code></div></div></pre>

This produces a `.tflite` model.

**Tips for Pi CPU:**

* prefer `yolov8n` or smaller
* keep `imgsz=320..640` depending on speed needs
* tune conf/NMS thresholds on-device

## 3) Export for Coral EdgeTPU (optional)

Coral requires:

1. TFLite model that is EdgeTPU-compatible (mostly INT8 ops)
2. compile with `edgetpu_compiler`

In practice you’ll do:

* quantization-aware or post-training INT8 quantization
* then compile:

<pre class="overflow-visible! px-0!" data-start="3395" data-end="3441"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"></div></pre>

<pre class="overflow-visible! px-0!" data-start="3395" data-end="3441"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"><div class="overflow-y-auto p-4" dir="ltr"><code class="whitespace-pre! language-bash"><span><span>edgetpu_compiler model.tflite -o .
</span></span></code></div></div></pre>

You’ll get `model_edgetpu.tflite`.

> Not every YOLO export will compile cleanly for EdgeTPU. If compilation fails,
>
> start with known EdgeTPU-friendly YOLO variants or use smaller heads / supported ops.

## 4) Validate outputs

`NAVscout` expects YOLO-like outputs and will postprocess with NMS.

If your model output differs, set `vision.output_layout` accordingly (see config docs).

---

## 5) Deployment

Copy to Pi/drone:

* `models/yolo/model.tflite` (CPU)
* optionally `models/yolo/model_edgetpu.tflite` (Coral)

<pre class="overflow-visible! px-0!" data-start="3963" data-end="4183"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"></div></pre>

`<span><span> </span><span>### `docs/INSTALL_EDGE.md ` (NEW)<span>``

```md
</span><span># Edge Installation (Pi / Drone Image) (docs/INSTALL_EDGE.md)</span><span>

</span><span>## Base packages (Debian/Pi OS)</span><span>
```bash
</span><span>sudo</span><span> apt-get update
</span><span>sudo</span><span> apt-get install -y libcamera-apps v4l-utils
</span></span>`
```

## TFLite C library

You need `libtensorflowlite_c.so`.

Options:

* install from your distro if available
* or build TensorFlow Lite C library once and package it into your drone image

Verify:

<pre class="overflow-visible! px-0!" data-start="4378" data-end="4423"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"></div></pre>

`<span><span>ldconfig -p | grep tensorflowlite</span></span>`

## Coral EdgeTPU (optional)

Install EdgeTPU runtime + compiler (for build machine). On Pi you need runtime library.

Verify:

<pre class="overflow-visible! px-0!" data-start="4550" data-end="4610"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"></div></pre>

<pre class="overflow-visible! px-0!" data-start="4550" data-end="4610"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"><div class="overflow-y-auto p-4" dir="ltr"><code class="whitespace-pre! language-bash"><span><span>lsusb | grep -i coral
ldconfig -p | grep edgetpu
</span></span></code></div></div></pre>

## Build NAVscout with TFLite

<pre class="overflow-visible! px-0!" data-start="4643" data-end="4714"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"></div></pre>

<pre class="overflow-visible! px-0!" data-start="4643" data-end="4714"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"><div class="overflow-y-auto p-4" dir="ltr"><code class="whitespace-pre! language-bash"><span><span>cargo build -p scout-cli --release --features vision-tflite
</span></span></code></div></div></pre>

With Coral delegate:

<pre class="overflow-visible! px-0!" data-start="4737" data-end="4821"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"></div></pre>

<pre class="overflow-visible! px-0!" data-start="4737" data-end="4821"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"><div class="overflow-y-auto p-4" dir="ltr"><code class="whitespace-pre! language-bash"><span><span>cargo build -p scout-cli --release --features vision-tflite,vision-coral
</span></span></code></div></div></pre>

## Camera capture modes

`NAVscout` supports:

* `v4l2-mjpeg` (UVC cameras)
* `libcamera-jpeg` (Pi camera) using `libcamera-still` as a robust capture helper

Configure in `configs/field_drone.toml`.
