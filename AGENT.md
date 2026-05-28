# Agent Notes

## User Play Folder

`C:\Users\Slump\OneDrive\デスクトップ\farming` is intended to be the user's real play/work folder, not a disposable test fixture.

Treat this folder as user-owned state:

- Do not delete the folder wholesale.
- Do not remove or overwrite user script files such as `rs_src\main.rs` unless the user explicitly asks.
- Do not treat files in this folder as temporary artifacts just because they were generated during setup.
- If cleanup is requested, prefer narrowly removing clearly generated build/cache output such as `rs_src\target` after confirming the target path.
- `farmrs.exe` and `farmrs.toml` in this folder are part of the user's normal runtime setup.
- The game Save folder configured by `farmrs.toml` is also user/game-owned and must not be cleaned automatically.

When testing farmrs internals, use a fresh folder under the system temp directory instead of `farming`.
