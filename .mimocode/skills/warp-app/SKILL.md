---
name: warp-app
description: "Create or modify BaramOS .warp application files. Covers screen blocks, variables, URI commands, layout patterns, and index.yaml registration."
---

# Warp App Development

Workflow for creating and modifying BaramOS .warp application files.

## When to use

- Creating a new .warp app (settings, theme, calculator, etc.)
- Modifying existing .warp layout or behavior
- Adding new screens to an existing app
- Updating `app/index.yaml` for new apps

## Reference files

Always read these first to understand conventions:
- `app/index.yaml` — app registration (name, type, icon)
- `app/settings.warp` — settings app with multi-screen navigation
- `app/demo.warp` — basic warp features demo
- `app/calc.warp` — calculator app with `calc{}` expressions

## .warp file structure

```
screen {id: (main)}
  -- variable declarations
  -- UI elements (tonalButton, card, hStack, etc.)
  oneClick { setScreen{settings} }  // navigation

screen {id: (settings)}
  tonalButton { label = "← 戻る" oneClick { setScreen{main} } }
  -- content
```

## Key conventions

- **Navigation**: `setScreen{id}` in `oneClick` blocks
- **Return button**: `tonalButton` at top of sub-screens with `setScreen{main}`
- **Variables**: `--varName = "value"` at screen top
- **Config reads**: `--os://display/pointer/size` for live XML config values
- **Layout**: `hStack { ... }` for horizontal layout with flex-wrap
- **Cards**: `card { ... }` for grouped content with rounded corners
- **Colors**: Use `config::get_color()` values, not hardcoded constants
- **Single-screen preference**: User prefers card-based single-screen over multi-screen navigation ("複数画面にせずカードでただ並べていって欲しい")

## index.yaml format

```yaml
apps:
  - name: settings
    type: warp-1
    icon: gear.svg
  - name: theme
    type: warp-1
    icon: palette.svg
```

## After creating/modifying .warp

1. Update `app/index.yaml` if new app
2. Run build-fix cycle (see build-fix skill)
3. Test on aarch64 target
