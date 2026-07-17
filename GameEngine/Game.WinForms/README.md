# Asteroids on Steroids — WinForms/GDI+ build (Windows only)

This is the **Windows** entry point. It runs the same game as `Game/` (SDL2 + SkiaSharp) but on a
WinForms window with a GDI+ renderer — **no SDL, Skia, or OpenGL dependency**.

## Requirements

- **Windows** (10/11).
- .NET 8 SDK **with the Windows Desktop workload** (`Microsoft.WindowsDesktop.App`). The stock SDK on
  Windows includes it; on Linux/macOS it does **not exist**, so this project **cannot be built or run
  off Windows**. Do not add it to a Linux build script — there is no solution file, so nothing pulls
  it in unless you name its `.csproj` explicitly.

## Build & run

From the repository's `GameEngine/` directory, on Windows:

```powershell
dotnet build Platform/WinForms/Engine.Platform.WinForms.csproj
dotnet build Game.WinForms/AsteroidsGame.WinForms.csproj
dotnet run   --project Game.WinForms
```

Assets resolve automatically: `GameConfigLoader.FindAssetsDir` walks up from the exe directory to
`GameEngine/Assets`, so no copying is needed as long as the project stays under `GameEngine/`.

## How it fits the engine

The engine is UI-toolkit-free; backends are separate assemblies implementing the PAL interfaces
(`IGameWindow`, `IRenderer`, optional `IPostEffects`). Two backends now exist:

```
AsteroidsEngine (net8.0, UI-free)
├── Platform/Sdl      (net8.0)          SdlGameWindow + SkiaRenderer   → Game/        (SDL exe)
└── Platform/WinForms (net8.0-windows)  WinFormsGameWindow + GdiRenderer → Game.WinForms/ (this)

GameCore (net8.0)  — states + GameHost (the shared bootstrap + fixed-timestep loop); no platform.
```

Both exes are a thin `Program.cs` that constructs their own window and call `GameHost.Run(window)`.

## Backend specifics & known caveats

- **Input**: `KeyCode`/`MouseButton` values equal `System.Windows.Forms.Keys`/`MouseButtons`, so keys
  map with a direct cast (no lookup table). `PollEvents()` pumps the Win32 queue via
  `Application.DoEvents()` — the engine owns the loop, WinForms does not call `Application.Run`.
- **Rendering**: GDI+ covers the whole `IRenderer` contract. Fonts use `GraphicsUnit.Pixel` so sizes
  match Skia's pixel `TextSize`; `FillPath` uses `FillMode.Winding` for a seamless multi-cell union.
- **`IPostEffects.Distort` (vortex / border warp) is APPROXIMATE here.** GDI+ has no triangle-mesh
  texture map, so it's faked with per-cell affine `Graphics.DrawImage` over a snapshot of the
  back-buffer (which is already a CPU `Bitmap`, so the snapshot is free). Gentle swirls read fine; it
  will not be pixel-identical to the Skia mesh warp and is CPU-bound, so keep the warp grids modest.
- **Not yet verified on hardware** — authored on Linux where it can't compile. First-run checklist:
  window opens fullscreen; WASD + mouse-aim/click + Q/E/R/G/F + Esc respond; HUD text/bars render at
  the right size (if text is too large, the font is being treated as points not pixels — check
  `GdiRenderer.GetFont`); asteroids fracture and render per-cell; vortex gust streaks + swirl warp and
  the red/grey edge tints appear; the window closes cleanly.
