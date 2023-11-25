# ascii-bomb-ecs-mp

WIP multiplayer demo of the [ascii-bomb-ecs](https://github.com/aleksa2808/ascii-bomb-ecs) game. Available as a [web build](https://aleksa2808.github.io/ascii-bomb-ecs-mp-manual/) (the build on the link is slightly modified, with a custom ggrs version (trying to prepare a PR for this)).

## Build

For both modes a [matchbox server](https://github.com/johanhelsing/matchbox/tree/main/matchbox_server) is needed. It can be run locally or used from `wss://match-0-6.helsing.studio` (thanks to [johanhelsing](https://github.com/johanhelsing)).

### Native

From the root folder run:

```bash
cargo run --release -- [--signal-server-address <SIGNAL_SERVER_ADDRESS>] [-n <NUMBER_OF_PLAYERS>]
```

### Web

From the root folder run:

```bash
wasm-pack build --target web --release
```

Then move the contents of `web` and the `assets` folder into `pkg`. After that, from the `pkg` folder you can start a local server by running:

```bash
# if basic-http-server is not yet installed
cargo install basic-http-server

basic-http-server
```

After that the game should be accessible on `localhost:4000`.
