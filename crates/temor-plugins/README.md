# temor-plugins

temor's plugins (PLAN D14, P11): the four capabilities, their messages, and the host side that
finds a plugin, runs it for one request and reads its answer. A plugin is a program that is run,
never linked; the wire is bayonet's.

## Trying a lens

`examples/lens_demo.rs` is a `block.lens` plugin: under a finished `ls` it draws a bar for each
line of the output, about six rows tall, and temor makes room for the picture under the block.

1. Build it (any target directory; this one is an example):

   ```sh
   CARGO_TARGET_DIR=/mnt/data/cargo/demo-temor cargo build -p temor-plugins --example lens_demo
   ```

2. Install it with its manifest. The manifest's file name must be its id, and `path` must be the
   program's absolute path (edit it if your home is elsewhere):

   ```sh
   mkdir -p ~/.local/share/temor/plugins
   cp /mnt/data/cargo/demo-temor/debug/examples/lens_demo ~/.local/share/temor/plugins/
   cp crates/temor-plugins/examples/lens-demo.toml ~/.local/share/temor/plugins/
   ```

3. Start temor (plugins are found when it starts), run `ls`, right-click the block (or open its
   "more" menu) and choose **Show with Line bars**. The picture appears under the block and the
   rows around it make way; **Hide** in the same menu takes it away.

To remove it, delete the two files from `~/.local/share/temor/plugins/`.
