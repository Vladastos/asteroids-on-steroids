// Asteroids on Steroids — SDL2 + SkiaSharp entry point.
//
//   WASD        thrust
//   Mouse       aim
//   Left-click  fire
//   Q/E/R       skills (dash / turbo / slow-mo)
//   Esc         quit
//
//   cd GameEngine/Game && dotnet run
//
// Backend-agnostic bootstrap + loop live in GameHost (AsteroidsGameCore); this file only
// constructs the concrete SDL window. The WinForms exe is the mirror of this file.

using AsteroidsGame;
using AsteroidsEngine.Platform.Sdl;

var (W, H) = SdlGameWindow.QueryDisplaySize();
using var window = new SdlGameWindow("Asteroids on Steroids", W, H, fullscreen: true);
GameHost.Run(window);
