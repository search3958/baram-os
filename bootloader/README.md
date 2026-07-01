# UEFI Bootloader Stub (GNU-EFI)
# License: BSD License (compatible with commercial use)

This directory contains a simple UEFI bootloader stub using GNU-EFI.

## Files

- `boot.c` - UEFI application entry point
- `Makefile` - Build configuration

## Requirements

- GNU-EFI library
- GCC for x86_64 or ARM64

## Building

```bash
# For x86_64
make ARCH=x86_64

# For ARM64
make ARCH=arm64
```

## Usage

Copy the generated `.efi` file to your EFI system partition:
```
/EFI/BOOT/BOOTX64.EFI  (for x86_64)
/EFI/BOOT/BOOTAA64.EFI (for ARM64)
```

## Note

This is a minimal example. For production use, consider:
- Adding error handling
- Supporting multiple boot options
- Implementing secure boot if needed
