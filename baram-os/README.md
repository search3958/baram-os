# Baram OS (바람 OS)

A simple UEFI-based hobby operating system with mouse pointer, keyboard input, and display output.

## License

This project is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.

All code in this project uses only open-source licenses that allow commercial use:
- MIT License
- BSD License

## Features

- ✅ UEFI support for x86_64 and ARM64
- ✅ Simple square mouse cursor display
- ✅ Keyboard input handling (arrow keys to move cursor)
- ✅ Framebuffer graphics output
- ✅ NOT a Linux clone - independent implementation

## Requirements

### For x86_64:
- GCC cross-compiler (`x86_64-w64-mingw32-gcc`) OR Clang
- GNU ld or lld linker
- llvm-objcopy (for creating EFI executable)
- QEMU (optional, for testing)
- OVMF.fd (UEFI firmware for QEMU)

### For ARM64:
- GCC cross-compiler (`aarch64-linux-gnu-gcc`) OR Clang
- GNU ld or lld linker
- llvm-objcopy
- QEMU (optional, for testing)
- QEMU_EFI.fd (UEFI firmware for QEMU)

### Installation on macOS:
```bash
brew install mingw-w64 llvm qemu
```

## Building

### For x86_64:
```bash
./build_x86_64.sh
```

### For ARM64:
```bash
./build_arm64.sh
```

## Running with QEMU

The build scripts automatically launch QEMU if it's installed. You can also run manually:

### x86_64:
```bash
qemu-system-x86_64 \
    -bios OVMF.fd \
    -drive format=raw,file=fat:rw:build \
    -m 512M
```

### ARM64:
```bash
qemu-system-aarch64 \
    -bios QEMU_EFI.fd \
    -drive format=raw,file=fat:rw:build \
    -m 512M \
    -cpu cortex-a57
```

## Project Structure

```
baram-os/
├── src/                    # Kernel source code
│   └── kernel.c           # Main kernel entry point
├── include/               # Header files
│   ├── kernel.h
│   ├── graphics.h
│   ├── keyboard.h
│   └── mouse.h
├── arch/                  # Architecture-specific code
│   ├── x86_64/
│   │   ├── graphics.c
│   │   ├── keyboard.c
│   │   ├── mouse.c
│   │   └── linker.ld
│   └── arm64/
│       ├── graphics.c
│       ├── keyboard.c
│       ├── mouse.c
│       └── linker.ld
├── efi/                   # UEFI boot code
│   ├── boot_x86_64.c
│   └── boot_arm64.c
├── build/                 # Build output directory
├── build_x86_64.sh       # x86_64 build script
├── build_arm64.sh        # ARM64 build script
└── README.md             # This file
```

## Controls

- **Arrow Keys**: Move the mouse cursor
- The cursor is displayed as a simple white square

## Notes

- This is a hobby OS project, not intended for production use
- The keyboard driver is simplified - full PS/2 or USB HID support would be needed for real hardware
- Graphics are limited to basic framebuffer operations

## Contributing

Feel free to fork and contribute! Please ensure any contributions use compatible open-source licenses (MIT, BSD, etc.).

## Acknowledgments

- UEFI Specification by UEFI Forum
- QEMU - Open source machine emulator
- EDK II - UEFI development kit
