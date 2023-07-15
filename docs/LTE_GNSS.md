# LTE + GNSS (docs/LTE_GNSS.md)

It focuses on GNSS ingest + safe uplink transport. Modem bring-up is left to system tooling.

## Debian/Pi OS deps

```bash
sudo apt-get update
sudo apt-get install -y modemmanager network-manager
sudo systemctl enable --now ModemManager NetworkManager

GNSS

Source options:

nmea-serial: read NMEA sentences from /dev/ttyUSB*

nmea-file: read a log file (for simulation/testing)

We parse:

GNRMC (time/speed/course/lat/lon)

GNGGA (satellites/HDOP/alt)

LTE

Use NetworkManager for session setup (APN) and verify with nmcli.
NAVscout assumes outbound connectivity exists and focuses on encrypted telemetry.




# LTE + GNSS Integration (docs/LTE_GNSS.md)

This project supports **CAT-4 LTE / 5G data uplink** and **GNSS positioning** on Raspberry Pi, with:

- minimal operational friction in the field
- encrypted telemetry by default
- offline-first spooling when uplink is degraded

## Supported modem families (practical)

We support modems by **interface class**, not by vendor name:

### 1) MBIM (preferred)

- Common on modern LTE/5G modules (including many M.2 Key-B modules).
- Best experience with NetworkManager/ModemManager.

### 2) QMI

- Common on Qualcomm-based modules (including many LTE hats).
- Works well but can be more finicky across kernels/carrier boards.

### 3) PPP (fallback)

- Legacy / emergency only.
- Higher CPU usage, worse reliability; not recommended for always-on tracking.

> Practical note: SIM7600X-like hats typically expose multiple `/dev/ttyUSB*` and/or a `cdc-wdm` device.
> M.2 Key-B LTE/5G modules often show up as **USB composite devices** behind the carrier board.

---

## System dependencies (Pi OS / Debian)

- `ModemManager` (modem control + GNSS in some cases)
- `NetworkManager` (IP session management)
- `mmcli`, `nmcli` for diagnostics

Suggested install:

```bash
sudo apt-get update
sudo apt-get install -y modemmanager network-manager
sudo systemctl enable --now ModemManager NetworkManager
```

# LTE + GNSS Integration (docs/LTE_GNSS.md)

This project supports **CAT-4 LTE / 5G uplink** and **GNSS positioning** on Raspberry Pi, with:

- minimal operational friction in the field
- encrypted telemetry by default
- offline-first spooling when uplink is degraded

---

## Supported hardware (practical, wide)

We support modems by **interface class**, not by vendor name.

### LTE / 5G modems

Typical examples:

- Waveshare SIM7600X-class HATs (LTE + GNSS)
- M.2 (NGFF) Key-B modules (SIM82xx / RM5xx-class) via carrier boards

What matters is how the modem presents itself to Linux:

1) **MBIM (preferred)**

- Common on modern LTE/5G modules.
- Best with ModemManager + NetworkManager.

2) **QMI**

- Common on Qualcomm-based modules.
- Works well; sometimes more kernel/driver sensitive.

3) **PPP (legacy fallback)**

- Higher CPU usage, lower reliability; not recommended for continuous use.

---

## System dependencies (Pi OS / Debian)

We lean on proven system components:

- `ModemManager` (modem control + optional GNSS location)
- `NetworkManager` (IP session management)
- `mmcli`, `nmcli` for diagnostics

Install:

```bash
sudo apt-get update
sudo apt-get install -y modemmanager network-manager
sudo systemctl enable --now ModemManager NetworkManager
```
