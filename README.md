# MyOS — UEFI ARM64 OS with Mouse + Keyboard + Graphics

A tiny but complete UEFI-based ARM64 operating system that boots in QEMU
(and on real Raspberry Pi 4/5 hardware) and demonstrates:

- **Graphics mode** via the UEFI Graphics Output Protocol (direct framebuffer writes)
- **Mouse pointer** via the **EFI Absolute Pointer Protocol** (usb-tablet)
  with fallback to Simple Pointer Protocol (usb-mouse)
- **Keyboard input** via UEFI Simple Text Input — arrow keys also move the
  cursor so you can test cursor drawing in headless QEMU
- **Live UI** showing mouse position, key events, FPS counter, and recent keys

The whole project builds and runs from a single `./build.sh` script on
macOS (Intel and Apple Silicon) and Linux.

---

## ✨ Features

| Capability | Implementation |
|---|---|
| UEFI boot | PE32+ ARM64 application built with `cargo +nightly` |
| Graphics | Direct framebuffer writes via Graphics Output Protocol (GOP) |
| Mouse (preferred) | **EFI Absolute Pointer Protocol** — usb-tablet, absolute (X, Y) |
| Mouse (fallback) | EFI Simple Pointer Protocol — usb-mouse, relative (ΔX, ΔY) |
| Keyboard | UEFI Simple Text Input Protocol — printable + scancode labels |
| Arrow keys | Move the cursor (handy for headless QEMU testing) |
| Cursor | 13×18 arrow sprite with drop shadow + background save/restore |
| Text | Built-in 8×16 VGA bitmap font (printable ASCII 0x20–0x7E) |
| UI | Title bar, status panels, recent-keys list, FPS counter |
| Targets | QEMU `virt` (Cortex-A72) and real Raspberry Pi 4/5 (with UEFI firmware) |

---

## 📋 Prerequisites

### macOS

```bash
# Install Homebrew (https://brew.sh) if you don't have it, then:
brew install qemu mtools rustup-init
rustup-init -y
```

> **Note:** Apple Silicon (M1/M2/M3) works exactly the same as Intel —
> QEMU emulates the ARM64 guest on either host architecture.

### Linux

```bash
# Debian/Ubuntu:
sudo apt install qemu-system-arm mtools curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Fedora:
sudo dnf install qemu-system-arm mtools curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify:
```bash
qemu-system-aarch64 --version    # QEMU 7.0+ recommended
cargo --version                   # any recent stable
mtools --version                  # for FAT image creation
```

---

## 🚀 Quick start

```bash
./build.sh           # build the OS, create FAT image, download UEFI firmware, boot in QEMU
```

The first run will:
1. Install the `aarch64-unknown-uefi` Rust target + nightly toolchain
2. Compile `bootaa64.efi` (≈33 KB)
3. Build a 64 MiB FAT disk image with the EFI binary at `EFI/BOOT/BOOTAA64.EFI`
4. Download `QEMU_EFI.fd` (AAVMF firmware, ≈3 MB) from a public mirror
5. Launch QEMU with mouse + keyboard + graphical display

When the QEMU window opens, you should see the MyOS UI with a white arrow
cursor in the centre. Move your mouse to drag the cursor; press any key to
add it to the recent-keys list.

Close the QEMU window (or Ctrl-C in the terminal) to exit.

---

## 🛠 Subcommands

```bash
./build.sh build      # only compile the EFI binary
./build.sh image      # build + create the FAT disk image
./build.sh firmware   # only download/verify UEFI firmware
./build.sh run        # only launch QEMU (assumes image + firmware present)
./build.sh clean      # cargo clean + remove disk image
./build.sh help       # show usage
```

---

## 🔧 Configuration (env vars)

| Variable | Default | Description |
|---|---|---|
| `QEMU_MACHINE` | `virt` | QEMU machine type |
| `QEMU_CPU` | `cortex-a72` | Emulated CPU (Pi 4 class) |
| `QEMU_RAM` | `1G` | Guest RAM size |
| `QEMU_DISPLAY` | `default` | QEMU display backend (`default`, `gtk`, `sdl`, `none`, `curses`) |

Example:
```bash
QEMU_RAM=2G QEMU_DISPLAY=gtk ./build.sh
```

---

## 📁 Project layout

```
myos/
├── build.sh                 # one-shot build + run script
├── Cargo.toml               # crate manifest (uses uefi-rs 0.38)
├── rust-toolchain.toml      # pins Rust nightly + UEFI target
├── .cargo/config.toml       # build-std config (needed for no_std + alloc)
├── README.md                # this file
├── scripts/
│   └── gen_font.py          # regenerates src/font_data.rs from the VGA font table
├── src/
│   ├── main.rs              # UEFI entry point + main loop
│   ├── gop.rs               # Graphics Output Protocol wrapper (framebuffer, pixels, rects)
│   ├── mouse.rs             # Simple Pointer Protocol driver (delta → absolute)
│   ├── keyboard.rs          # Simple Text Input driver (printable + scancodes)
│   ├── cursor.rs            # 13×18 mouse cursor sprite + background save/restore
│   ├── font.rs              # 8×16 bitmap font API
│   ├── font_data.rs         # generated font table (95 glyphs, ASCII 0x20–0x7E)
│   └── ui.rs                # text rendering + small FmtBuf helper
└── runtime/                 # QEMU_EFI.fd and osdisk.img land here (gitignored)
```

---

## 🍓 Running on real Raspberry Pi 4/5

The same `bootaa64.efi` boots on real Raspberry Pi hardware. To deploy:

1. **Get Pi UEFI firmware** from
   https://github.com/pftf/RPi4/releases (Pi 4) or
   https://github.com/worproject/rpi5-uefi (Pi 5).
   Copy the `firmware/` contents to a FAT32-formatted SD card root.

2. **Add the OS** by copying `target/aarch64-unknown-uefi/release/bootaa64.efi`
   to `EFI/BOOT/BOOTAA64.EFI` on the same SD card.

3. **Boot** the Pi with the SD card inserted. The UEFI firmware will find
   `EFI/BOOT/BOOTAA64.EFI` and launch MyOS.

> The QEMU `virt` machine and the Pi UEFI firmware both implement the
> standard UEFI protocols MyOS relies on (GOP, Simple Pointer, Simple
> Text Input), so no code changes are required.

---

## 🧪 How it works

### Boot flow

1. UEFI firmware (AAVMF in QEMU, RPi firmware on Pi) loads `BOOTAA64.EFI`
   from the FAT partition.
2. The Rust `#[entry]` macro generates the UEFI entry point. `main()`
   calls `uefi::helpers::init()` to set up the global allocator, logger,
   and panic handler.
3. `Screen::take()` opens the Graphics Output Protocol and selects the
   highest-resolution mode available (typically 1024×768 in QEMU).
4. `Keyboard::is_present()` and `mouse::mouse_present()` locate the
   Simple Text Input and Simple Pointer protocols via the UEFI handle
   database.
5. The main loop polls input, repaints the UI, and renders the cursor
   ~120 times per second.

### Drawing pipeline

- Each pixel is written directly to the framebuffer via volatile stores.
- The cursor is composited last; its background is saved first and
  restored before the next repaint to avoid smearing.
- The 8×16 font is rendered one glyph at a time, with both foreground
  and background colours (no alpha blending — fast and simple).

### Mouse cursor

The cursor is a 13×18 1-bit arrow sprite. Before drawing, the rectangle
under the cursor is copied into a save buffer. On the next frame, that
buffer is blitted back before the cursor is redrawn at its new position.
This gives clean, flicker-free cursor movement without a full repaint.

---

## 🐛 Troubleshooting

**`qemu-system-aarch64: failed to find romfile "efi-virtio.rom"`**
QEMU can't find its data files. Make sure you installed `qemu-system-arm`
(not just `qemu-system-aarch64` from a static build). On macOS via Homebrew
this is automatic.

**`error: failed to find romfile "vgabios-ramfb.bin"`**
Same as above — the QEMU data directory is missing. Reinstall QEMU.

**Blue or "Display output is not active" screen**
The UEFI firmware couldn't initialise GOP. Make sure you're using the
`-device ramfb` line in `build.sh` (it's the default). Some QEMU builds
don't ship a virtio-gpu driver in the firmware; `ramfb` works everywhere.

**`Mouse: Not present` in the UI**
The QEMU `virt` machine exposes a USB mouse through the XHCI controller.
Make sure your QEMU command line includes `-device qemu-xhci -device usb-tablet`.
On real Pi hardware, the UEFI firmware exposes a mouse via USB automatically.

**Mouse driver loads but events count stays at 0 (headless QEMU)**
This is a **known limitation of QEMU's HMP `mouse_move` command** — it
sends events to the legacy PS/2 mouse input layer, which the UEFI USB
HID driver doesn't see.  Two workarounds:

1. **Use the QEMU GUI window**: run `./build.sh` without overriding
   `QEMU_DISPLAY`.  Click inside the QEMU window to grab the mouse, then
   move it — the OS will see the events.
2. **Use arrow keys**: ↑↓←→ also move the cursor (12 px per press) — this
   works in headless QEMU and lets you verify the cursor drawing code.

On real Raspberry Pi 4/5 hardware, a USB mouse works without any of these
workarounds.

**Build error: `can't find crate for 'core'`**
You're missing the `aarch64-unknown-uefi` target. Run:
```bash
rustup target add aarch64-unknown-uefi --toolchain nightly
rustup component add rust-src --toolchain nightly
```

**`error[E0433]: cannot find type ...` after upgrading `uefi` crate**
The `uefi` crate's API has changed between minor versions. This project
pins `uefi = "0.38"`. If you upgrade, expect to need code changes.

---

## 📜 License

The OS source code in this project is licensed under MIT.

The bundled 8×16 VGA font (`src/font_data.rs`, generated by
`scripts/gen_font.py`) is derived from the public-domain IBM VGA font.

The `uefi` crate (https://github.com/rust-osdev/uefi-rs) is licensed
under MIT OR Apache-2.0.
