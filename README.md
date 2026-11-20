# drover-desktop

The native desktop window of Drover, drawn with GPUI (binary `drover-native`). It is a client of the Drover daemon
from [drover-core](https://github.com/doquangthanh/drover-core): it connects over the local RPC / WebSocket protocol
(`drover-proto`) and draws terminals, agents, files, changes, inbox, settings and machines itself.

- Depends on `drover-core` and `drover-proto` as git dependencies at a tag (`rust/crates/drover-desktop-native/Cargo.toml`).
  Move the tag when you move to a new core protocol.
- `rust/vendor/` holds two patched GPUI dependencies (`xattr`, `zed-xim`).
- Build: `cd rust && cargo build --release -p drover-desktop-native`. Needs the GPUI system libraries (see `.github/workflows/ci.yml`).
- Run: start the daemon from drover-core (`dr server start`), then `drover-native`.
