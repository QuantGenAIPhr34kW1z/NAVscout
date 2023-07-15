# Matek H743-WING V3 Flight Controller Interface (docs/FC_MATEK_H743_WING_V3.md)

NAVscout does NOT “drive” the STM32 directly.
Instead it talks to the flight stack running on the FC (ArduPilot / iNav) over a serial telemetry link.

## Recommended protocol: MAVLink over UART

- Stable ecosystem (GCS tooling, well-known failsafes)
- Good match for Pi companion computers

ArduPilot explicitly documents the Matek H743 Wing family and its UART/telemetry capabilities. :contentReference[oaicite:2]{index=2}
Matek product docs confirm multiple UARTs and typical UAV use. :contentReference[oaicite:3]{index=3}

## Wiring (generic)

Pick a free TELEM/UART port on the FC:

- FC TX -> Pi RX
- FC RX -> Pi TX
- FC GND -> Pi GND
  Power: do NOT back-power incorrectly; follow your FC power guidance.

On the Pi, typical serial devices:

- `/dev/serial0` (GPIO UART)
- `/dev/ttyAMA0` / `/dev/ttyS0` depending on model/config
- `/dev/ttyUSB0` if via USB-UART

## Firmware setup

### ArduPilot

- Set the chosen SERIALx_PROTOCOL = MAVLink2
- Set SERIALx_BAUD to match config (e.g. 57600 or 115200)
- Ensure your telemetry port is enabled and not used by RC receiver

### iNav

- Enable MAVLink telemetry on a UART
- Make sure it emits heartbeats

## What NAVscout sends (intentionally minimal)

- Heartbeat (companion)
- Command: NAV_RETURN_TO_LAUNCH (RTL)

This is bounded to avoid dangerous control loops.
If you want full “offboard” control later, do it behind a hard safety layer.
