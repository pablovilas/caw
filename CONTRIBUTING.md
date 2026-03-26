# Contributing to caw

## Development

```sh
just setup    # Configure git hooks (pre-push runs lint + tests)
just ci       # Run clippy + tests
just build    # Build release binary
just bundle   # Build macOS .app bundle
just run      # Run in dev mode
```

## Adding a New Plugin

1. Create the crate:
   ```sh
   cargo new crates/caw-plugin-<name> --lib
   ```

2. Add `caw-core` as a dependency in the new crate's `Cargo.toml`:
   ```toml
   [dependencies]
   caw-core = { path = "../caw-core" }
   ```

3. Implement the `IPlugin` trait — only `discover()` and `read_session()` are required:
   ```rust
   use async_trait::async_trait;
   use caw_core::{IPlugin, RawInstance, RawSession};

   pub struct MyPlugin;

   #[async_trait]
   impl IPlugin for MyPlugin {
       fn name(&self) -> &'static str { "my-plugin" }
       fn display_name(&self) -> &'static str { "My Plugin" }

       async fn discover(&self) -> anyhow::Result<Vec<RawInstance>> {
           // Find running processes or session files
           todo!()
       }

       async fn read_session(&self, id: &str) -> anyhow::Result<Option<RawSession>> {
           // Parse session data for a discovered instance
           todo!()
       }
   }
   ```

4. Register in `crates/caw/src/main.rs`:
   ```rust
   registry.register(Arc::new(MyPlugin::new()));
   ```

5. Add the crate to workspace members in the root `Cargo.toml`:
   ```toml
   [workspace]
   members = [
       # ...
       "crates/caw-plugin-<name>",
   ]
   ```

6. Run `just ci` to verify everything passes, then submit a PR.
