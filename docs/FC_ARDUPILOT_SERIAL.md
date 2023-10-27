# ArduPilot (ChibiOS) Serial / Telemetry for Companion (docs/FC_ARDUPILOT_SERIAL.md)

Goal: enable MAVLink2 on a telemetry UART so NAVscout can:

- read HEARTBEAT
- request RTL safely (COMMAND_LONG NAV_RETURN_TO_LAUNCH)

ArduPilot serial port parameters are documented (SERIALx_*). :contentReference[oaicite:3]{index=3}

## Typical setup

Pick the serial port index `x` that corresponds to your chosen UART (TELEM port).

Set:

- `SERIALx_PROTOCOL = 2`  (MAVLink2) :contentReference[oaicite:4]{index=4}
- `SERIALx_BAUD = 57` for 57600, or `115` for 115200 (see ArduPilot docs UI) :contentReference[oaicite:5]{index=5}
- disable flow control unless you wire CTS/RTS (`BRD_SERx_RTSCTS = 0`) :contentReference[oaicite:6]{index=6}

## Autodetect strategy used by NAVscout

NAVscout probes candidate Linux serial devices and common bauds,
waiting briefly for MAVLink HEARTBEAT. This matches MAVLink expected message flow. :contentReference[oaicite:7]{index=7}

Run:
`scout --config configs/field_drone.toml fc autodetect`
