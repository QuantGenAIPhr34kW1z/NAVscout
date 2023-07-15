# Training YOLO for your objects (docs/TRAINING_YOLO.md)

This guide trains a custom detector (e.g., "btr", "tnk", "drone", "person", "car", "bus") and exports to ONNX for Raspberry Pi.

> Privacy note: only record/train where you have permission. Avoid capturing neighbors/public areas.

---

## 1) Collect data

You need images/videos covering:

- lighting: sun/cloud/dusk
- motion blur: running, fast turns
- occlusion: behind bushes, partial views
- distance scales: near + far

Tips:

- Extract frames from video at ~2–5 fps for variety.
- Keep at least 300–1000 labeled images for a strong single-class model.

---

## 2) Labeling

Tools:

- labelImg (local)
- CVAT (self-hosted)
- Roboflow (hosted, convenient)

Label bounding boxes for your classes.

Export format:

- YOLO (txt) labels

---

## 3) Train with Ultralytics YOLO (recommended)

On a GPU machine:

Install:

```bash
python3 -m venv .venv
. .venv/bin/activate
pip install ultralytics
```



Train:

<pre class="overflow-visible! px-0!" data-start="9294" data-end="9384"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"></div></pre>

<pre class="overflow-visible! px-0!" data-start="9294" data-end="9384"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"><div class="overflow-y-auto p-4" dir="ltr"><code class="whitespace-pre! language-bash"><span><span>yolo detect train model=yolov8n.pt data=data.yaml imgsz=640 epochs=80 batch=16
</span></span></code></div></div></pre>

Notes:

* start with `yolov8n` (nano) for Pi friendliness
* increase `imgsz` only if needed

---

## 4) Export to ONNX

<pre class="overflow-visible! px-0!" data-start="9504" data-end="9588"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"></div></pre>

<pre class="overflow-visible! px-0!" data-start="9504" data-end="9588"><div class="contain-inline-size rounded-2xl corner-superellipse/1.1 relative bg-token-sidebar-surface-primary"><div class="overflow-y-auto p-4" dir="ltr"><code class="whitespace-pre! language-bash"><span><span>yolo </span><span>export</span><span> model=runs/detect/train/weights/best.pt format=onnx opset=12
</span></span></code></div></div></pre>

For edge speed:

* use a smaller model (n/s)
* consider INT8 quantization (advanced; depends on runtime)

---




---
## 5) Validate


Run inference on a sample set and measure:


* precision/recall
* false positives in bushes/shadows
* fast motion failure cases


Then add more data for the failure cases and retrain.
---
## 6) Tracking fast objects (practical)

Detection ≠ tracking. For fast motion:

* use higher FPS camera capture where possible
* keep shutter faster (better lighting helps)
* tracker should use motion model (Kalman) and allow short occlusions
* consider “burst mode”: temporarily increase compute when target is found
