# ascii-bomb-ecs-mp

Multiplayer experiment of the [ascii-bomb-ecs](https://github.com/aleksa2808/ascii-bomb-ecs) game that uses peer-to-peer rollback networking. Available natively or as a [web build](https://aleksa2808.github.io/ascii-bomb-ecs-mp/).

## Configuration

A [matchbox server](https://github.com/johanhelsing/matchbox/tree/main/matchbox_server) is needed to connect players. Without any configuration the one at `wss://match-0-6.helsing.studio` is used (thanks to [johanhelsing](https://github.com/johanhelsing)).

Additionally, if a direct connection cannot be made between clients, a TURN relay server is used through which all communication happens. The default TURN server is hosted in Frankfurt and has limited bandwidth, which can translate to high ping times for clients that are far away or unavailability if the monthly bandwidth is depleted.

## Web build

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
