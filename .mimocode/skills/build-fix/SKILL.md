---
name: build-fix
description: "BaramOS build-compile-fix cycle: build for both targets, parse errors, fix code, repeat until clean. Ensures both aarch64 and x86_64 UEFI targets compile before declaring done."
---

# Build-Fix Cycle

Standardized workflow for BaramOS build-compile-fix cycles. Ensures both targets are tested before declaring work complete.

## When to use

- After any code change in `crates/`
- When user reports a compile error
- Before declaring any task "done" (user rule: "テストはしたんか")
- When user demands "x86_64もちゃんとビルドテストしろと"

## Build commands

### Quick check (fast feedback)
```bash
cd /Users/cheontaerang/Documents/GitHub/baram-os && cargo check 2>&1 | grep -E "^error" | head -10
```

### Full release build — aarch64 (primary target)
```bash
cd /Users/cheontaerang/Documents/GitHub/baram-os && cargo +nightly build --release --target aarch64-unknown-uefi 2>&1 | tail -5
```

### Full release build — x86_64 (must also pass)
```bash
cd /Users/cheontaerang/Documents/GitHub/baram-os && cargo +nightly build --release --target x86_64-unknown-uefi 2>&1 | grep -E "^error|Finished" | head -10
```

### Full build + image creation (aarch64)
```bash
cd /Users/cheontaerang/Documents/GitHub/baram-os && cargo +nightly build --release --target aarch64-unknown-uefi 2>&1 | tail -3 && rm -f runtime/osdisk.img && ./build.sh image 2>&1 | tail -3
```

## Workflow

1. **After code changes**: Run `cargo check` first for fast feedback
2. **Fix errors**: Parse error messages, read relevant source, apply fix
3. **Re-check**: Run `cargo check` again to verify fix compiles
4. **Full build**: Run full release build for aarch64
5. **x86_64 build**: Run full release build for x86_64 (user explicitly requires this)
6. **Image creation**: Only after both targets compile clean
7. **Report**: State what was changed and confirm both targets compile

## Rules

- NEVER declare "done" without at least a `cargo check` passing
- ALWAYS test both aarch64 AND x86_64 targets
- If one target has a pre-existing error unrelated to changes, note it but still verify the other target
- User language: ビルド成功/失敗, ターゲット名を明記
