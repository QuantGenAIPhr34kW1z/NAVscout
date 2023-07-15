# NAVscout 🛰️🦀

### AI-Powered Autonomous Drone Navigation & Object Tracking for Raspberry Pi 5

<div align="center">

**`Edge Intelligence` • `Real-Time Tracking` • `Secure Telemetry` • `Slow Link Optimized`**

[![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg?style=for-the-badge)](LICENSE)
[![RPI5](https://img.shields.io/badge/Raspberry%20Pi%205-C51A4A?style=for-the-badge&logo=Raspberry-Pi)](https://www.raspberrypi.org/)
[![TFLite](https://img.shields.io/badge/TensorFlow%20Lite-FF6F00?style=for-the-badge&logo=tensorflow&logoColor=white)](https://www.tensorflow.org/lite)
[![Status](https://img.shields.io/badge/status-beta-orange.svg?style=for-the-badge)](https://github.com/QuantGenAIPhr34kW1z/NAVscout)

[Features](#-key-features) • [Quick Start](#-quick-start) • [Architecture](#-architecture) • [Documentation](#-documentation) • [Contributing](#-contributing)

</div>

---

## 🎯 Mission

NAVscout transforms Raspberry Pi 5 into an **intelligent autonomous drone navigator** with on-device AI vision, persistent object tracking, and robust operation over **slow, unreliable data links** (2G/3G/LTE).

Perfect for:

- 🐕 **Search & Rescue**: Track and follow targets in large areas with fast motion tolerance
- 🌿 **Terrain Research**: Low-power, privacy-first monitoring with minimal data footprint
- 📍 **Asset Recovery**: Locate and track objects within permitted zones

---

## ✨ Key Features

### 🤖 AI Vision & Tracking

- **TensorFlow Lite Inference** - Optimized for RPI5 CPU with optional Coral EdgeTPU acceleration
- **YOLO Object Detection** - Custom-trained models for your specific use case
- **Multi-Object Tracking** - IOU association with constant-velocity prediction and occlusion handling
- **Adaptive Power Modes** - Scan/Track/Burst modes balance battery life vs tracking quality
- **ROI Optimization** - Dynamic region-of-interest cropping focuses compute on target area

### 🧭 GNSS Navigation & Safety

- **Real-Time GNSS** - NMEA parser with fix quality validation (sats, HDOP, age)
- **Geofencing** - Corridor-based routing + polygon zone enforcement
- **Return-To-Home (RTH)** - Automatic failsafe on: link loss, GNSS degrade, battery low, thermal
- **Mission State Machine** - TransitToZone → OperateInZone → RTH → Land with validated transitions

### 📡 Connectivity (Slow Link Ready)

- **LTE/5G Uplink** - Works on 2G fallback with adaptive rate limiting
- **Encrypted Telemetry** - XChaCha20-Poly1305 AEAD end-to-end encryption
- **Offline-First** - Automatic spool-and-flush with bounded disk usage
- **Certificate Pinning** - MITM protection for remote endpoints (configurable)

### ✈️ Flight Controller Integration

- **MAVLink Protocol** - ArduPilot/PX4 compatible via serial
- **Auto-Detection** - Probes multiple ports/bauds for heartbeat
- **Safety-First** - Only allows RTL (return-to-launch) and HOLD commands
- **Heartbeat Monitoring** - Validates FC connectivity before sending commands

### 🔒 Security & Privacy

- **Encrypted At-Rest** - All telemetry and recordings use AEAD encryption
- **Key Rotation** - Secure key lifecycle with passphrase wrapping
- **No Cloud Streaming** - Fully on-device processing
- **Minimal Retention** - Configurable data expiry (default: 3 days)
- **Strict Permissions** - Device keys protected with 0600 Unix permissions

---

## 🚀 Quick Start

### Prerequisites

**Hardware:**

- Raspberry Pi 5 (4GB+ RAM recommended)
- Pi Camera v2/v3 or USB UVC camera
- Optional: LTE/5G modem (MBIM/QMI compatible)
- Optional: GNSS receiver (NMEA via serial or ModemManager)
- Optional: Coral USB EdgeTPU for 10x inference speedup

**Software:**

```bash
# Install system dependencies (Raspberry Pi OS)
sudo apt-get update
sudo apt-get install -y libcamera-apps v4l-utils
sudo apt-get install -y modemmanager network-manager  # For LTE

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Build

```bash
# Clone the repository
git clone https://github.com/QuantGenAIPhr34kW1z/NAVscout.git
cd navscout

# Build with TFLite (CPU inference)
cargo build -p scout-cli --release --features vision-tflite

# OR build with Coral EdgeTPU support
cargo build -p scout-cli --release --features vision-tflite,vision-coral
```

### Initialize

```bash
# 1. Initialize encryption keys
./target/release/scout --config configs/field_drone.toml keys init

# 2. Run system diagnostics
./target/release/scout --config configs/field_drone.toml doctor

# 3. Inspect your TFLite model (verify tensor shapes)
./target/release/scout --config configs/field_drone.toml vision inspect
```

### Run

```bash
# Start the full pipeline (vision + nav + uplink + FC integration)
./target/release/scout --config configs/field_drone.toml run
```

---

## 📐 Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        NAVscout Pipeline                        │
└─────────────────────────────────────────────────────────────────┘

┌──────────────┐    ┌───────────────┐    ┌──────────────┐
│   Camera     │───▶│  TFLite YOLO  │───▶│   Tracker    │
│  (libcamera/ │    │  + NMS Filter │    │  (IOU + KF)  │
│   V4L2)      │    │               │    │              │
└──────────────┘    └───────────────┘    └──────────────┘
                            │                     │
                            ▼                     ▼
                    ┌──────────────────────────────────┐
                    │  Power Controller                │
                    │  (Scan/Track/Burst Modes)        │
                    └──────────────────────────────────┘
                                   │
                                   ▼
        ┌──────────────┐    ┌─────────────────┐    ┌────────────┐
        │  GNSS Parser │───▶│  Nav Engine     │───▶│ MAVLink FC │
        │  (NMEA)      │    │  (Geofence/RTH) │    │ (RTL/HOLD) │
        └──────────────┘    └─────────────────┘    └────────────┘
                                   │
                                   ▼
                          ┌─────────────────┐
                          │  Telemetry      │
                          │  (Encrypted)    │
                          └─────────────────┘
                                   │
                                   ▼
                          ┌─────────────────┐
                          │  LTE Uplink     │
                          │  (TLS + Spool)  │
                          └─────────────────┘
```

### Crates Structure

- **`scout-cli`** - Main entry point, config loading, run loop orchestration
- **`scout-vision`** - TFLite detector, tracker, power modes, camera capture
- **`scout-nav`** - GNSS parsing, geofence validation, RTH logic
- **`scout-uplink`** - TLS client, encrypted spool, link health monitoring
- **`scout-crypto`** - AEAD encryption, key management, rotation
- **`scout-proto`** - Telemetry frame schema and versioning
- **`scout-fc`** - MAVLink flight controller adapter with safety constraints

---

## ⚙️ Configuration

All behavior is controlled via TOML config files. See `configs/field_drone.toml` for a complete example.

### Example: Track an Object in a Zone

```toml
[vision]
enable = true
backend = "tflite"
model_path = "models/yolo/btr-detector.tflite"
conf_threshold = 0.35
nms_iou_threshold = 0.45

[tracking]
enable = true
target_class = "dog"           # Lock onto this class
lock_min_conf = 0.40
max_age_frames = 15            # Tolerate 15-frame occlusion

[power]
mode = "scan"                  # Start in low-power mode
scan_infer_every_n = 6         # Run inference every 6 frames when idle
track_infer_every_n = 2        # Increase rate when locked
burst_seconds = 2.0            # Full-rate burst on target found

[nav]
home = { lat = 48.000000, lon = 2.000000, alt_m = 35.0 }
max_radius_m = 1200.0          # Hard limit from home

[nav.zone]
zone_polygon = [             # Must stay within this area
  { lat = 48.001100, lon = 2.002100 },
  { lat = 48.001200, lon = 2.002400 },
  { lat = 48.001000, lon = 2.002500 },
  { lat = 48.000900, lon = 2.002200 }
]

[rth]
grace_link_loss_s = 20         # RTH after 20s of link loss
gnss_bad_fix_s = 8             # RTH if GNSS bad for 8s
battery_low_pct = 22           # RTH at 22% battery
```

---

## 📚 Documentation

- **[Training Your Model](docs/TRAINING_YOLO.md)** - How to train custom YOLO detectors for your objects
- **[TFLite Export Guide](docs/TRAINING_EXPORT_TFLITE.md)** - Export YOLO to TFLite for Raspberry Pi
- **[LTE/GNSS Setup](docs/LTE_GNSS.md)** - Configure ModemManager, NMEA sources, APN settings
- **[Flight Safety](docs/FLIGHT_SAFETY.md)** - Geofence, RTH triggers, and mission states
- **[ArduPilot Integration](docs/FC_ARDUPILOT_SERIAL.md)** - Serial telemetry setup for ArduPilot
- **[Hardware Guides](docs/)** - Matek H743 Wing v3 wiring, Coral EdgeTPU setup

---

## 🎓 Training Custom Models

NAVscout uses TensorFlow Lite for on-device inference. You can train your own YOLO models:

```bash
# 1. Collect and label your data (CVAT, labelImg, Roboflow)
# 2. Train with Ultralytics YOLO
python3 -m venv .venv && source .venv/bin/activate
pip install ultralytics

yolo detect train model=yolov8n.pt data=data.yaml imgsz=640 epochs=80

# 3. Export to TFLite
yolo export model=runs/detect/train/weights/best.pt format=tflite

# 4. Verify tensor layout
./target/release/scout --config your_config.toml vision inspect
```

See full guide: **[docs/TRAINING_YOLO.md](docs/TRAINING_YOLO.md)**

---

## 🔧 Commands Reference

```bash
# Run main pipeline
scout run

# Validate configuration and system readiness
scout doctor

# Initialize encryption keys
scout keys init

# Rotate encryption keys
scout keys rotate

# Inspect TFLite model tensor shapes
scout vision inspect

# Auto-detect flight controller serial port
scout fc autodetect

# Check flight controller status
scout fc status
```

---

## 🛡️ Security & Privacy

NAVscout is designed with **privacy-by-default** and **security-first** principles:

- ✅ **End-to-end encryption** for all telemetry (XChaCha20-Poly1305)
- ✅ **No cloud streaming** - all processing happens on-device
- ✅ **Short retention** - configurable expiry (default: 3 days)
- ✅ **Encrypted at-rest** - spooled data uses AEAD encryption
- ✅ **Certificate pinning** - protect against MITM on slow links
- ✅ **Key rotation** - secure lifecycle management
- ✅ **Minimal data** - only essential metadata leaves device

**Hardening Checklist:**

1. Run NAVscout as dedicated user with restricted permissions
2. Set key files to `0600` (enforced automatically on Unix)
3. Enable firewall with outbound-only rules
4. Disable password SSH (use keys)
5. Keep Raspberry Pi OS updated

---

## 📊 Performance Benchmarks (RPI5)

| Configuration           | Inference Time | FPS      | Power Draw |
| ----------------------- | -------------- | -------- | ---------- |
| YOLOv8n (CPU)           | ~320ms         | 3.1 FPS  | 4.2W       |
| YOLOv8n (Coral EdgeTPU) | ~28ms          | 35.7 FPS | 6.8W       |
| YOLOv8s (CPU)           | ~780ms         | 1.3 FPS  | 5.1W       |

> Benchmarked on RPI5 8GB, 640x640 input, ambient temp 25°C

---

## 📜 License

Copyright © 2025 NAVscout Contributors

---

## 🙏 Acknowledgments

- **[Ultralytics YOLO](https://github.com/ultralytics/ultralytics)** - Object detection framework
- **[TensorFlow Lite](https://www.tensorflow.org/lite)** - On-device ML inference
- **[MAVLink](https://mavlink.io/)** - Drone communication protocol
- **[Rust Community](https://www.rust-lang.org/)** - Memory-safe systems programming

---

## 📞 Support & Community

- 💬 **Discussions**: [GitHub Discussions](https://github.com/QuantGenAIPhr34kW1z/NAVscout/discussions)
- 🐛 **Issues**: [GitHub Issues](https://github.com/QuantGenAIPhr34kW1z/NAVscout/issues)
- **If you like this project, give it a ⭐!**
---

<div align="center">

**Made with ❤️ and 🦀 for autonomous flight**

[⬆ Back to Top](#navscout-)

</div>
