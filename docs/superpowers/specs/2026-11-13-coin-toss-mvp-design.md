# Coin Toss MVP Design (milestone 1)

- Date: 2026-11-13
- Status: Approved (the round-1 review findings, and the five advisories given at approval, are folded into the decisions in §1 and into the sections they touch)
- **Scope**: `crates/padparadscha-common`, `crates/padparadscha-table`, `classics/crates/padparadscha-classics-core`, `classics/desktop`, `classics/headless`, `classics/mobile`, `classics/web`
- **References**: `BOOTSTRAP.md` at the repository root, `corundum-framework-platform` (`dirs`, per-OS plumbing), `corundum-framework-gui` (`look` / `theme` / `i18n` / Lucide icons. egui widgets; eframe is not added), `sapphire-agent-gui` (adds bevy_egui as a bevy plugin; the shape of an app in this repository), sapphire-agent's ACP server (`GET /acp`, `POST /rpc`), sapphire-agent's MCP over ACP (`"type": "acp"`, `acp__<serverName>__<tool>`)
- **This spec covers milestone 1 only.** Classics proper and the sandbox are each written up in their own spec.

## 1. Purpose and Scope

Run the tabletop game engine end to end on one thing: coin toss. Coin toss is not a game in its own right — it is a **bonus** capability the engine carries, listed as one entry of the classics collection.

### What this milestone verifies

Whether that feeling of "playing together" comes out of this shape. The bar is not how well the numbers come out — **what the author felt is the last word** (§11). The target is a two-player game: one user and one AI (as `BOOTSTRAP.md` already assumes).

### Decided (by the author)

| Decision | What |
|---|---|
| Crate layout **A** (2026-11-13) | Coin toss is implemented as **an engine capability on the padparadscha-table side**. classics-core only names it, as one entry of the collection |
| Physics **rapier3d** (2026-11-13) | We know it is heavy. Going with it for now |
| wasm **from the first slice** (2026-11-13) | Not deferred |
| Session **one session, shared** (2026-11-13) | One ACP session. Chat and game are the same room |
| **bevy is the axis** (approved by the author in review round 1) | Both the engine and the app window are bevy. egui is **widgets** (the chat panel, the title, the settings) riding inside the bevy window through bevy_egui. eframe and an "egui shell" are not the architecture. bevy goes into this milestone (not deferred) |
| **command order** (approved by the author in review round 1) | This milestone uses a plain FIFO queue. Strict alternating turns are left as a later, game-side concern (§5.1) |

### Terms (the contract for reading this spec and the implementation plan that follows)

| Term | What it means in this spec |
|---|---|
| client | The side that produces a window: desktop, mobile, web |
| game server | The padparadscha-side server. It is the binary itself under headless; under desktop and mobile it lives inside the client |
| the agent | sapphire-agent. **The ACP server**, the peer the game server connects to |
| conversation | The flow of the one shared ACP session (the user's utterances, the agent's replies, the game-originated lines) |
| command | What a peer issues (`toss` / `log`). Once accepted it becomes a `seq`-tagged row |
| `GameEvent` | The state-change row a command produces |
| log | The one append-only sequence that accumulates commands and `GameEvent`s under `seq`. **The canonical form of the game state** (§7.3) |
| feed | What the game server streams to clients (log appends and conversation content) |

### Out of scope (left out on purpose)

- Scoreboards, standings, records. Coin toss is a bonus, not another game being added.
- Other tabletop games (cards, original TRPG systems). Those are written up in the classics-proper spec.
- Dice. Kept only in the shape of being loadable onto the same read-out mechanism later. Not implemented this time.
- Strict alternating turns, whose turn it is, waiting. This time it is only a FIFO queue (§5.1).
- Adding settings items. Language, theme and the server URL are already in corundum-framework's settings.
- Multiple users, auth and permissions work, voice. Right now it is one person, and the other is an AI.
- Animation, sound and effects work. Things move only as much as is needed to show they move.

## 2. Overall Shape

### 2.1 Crates and Division of Labor

| Crate | Role | Depends on |
|---|---|---|
| `padparadscha-common` | The app's foundation. Shipped as a bevy plugin: binds the ACP session, renders the title, the settings and the chat panel, registers the MCP tools. The overall flow shared by classics and the sandbox | bevy (`default-features = false`), bevy_egui, corundum-framework-gui (`default-features = false`; without eframe), corundum-framework-platform, agent-client-protocol, rmcp, tokio |
| `padparadscha-table` | The tabletop game engine, running as a bevy plugin: the table, the command queue, rigid bodies and rapier3d, state changes turned into `GameEvent`s, and the layer that turns those into conversation lines. **Coin toss is the engine capability placed here** | padparadscha-common, rapier3d, serde, tokio |
| `padparadscha-classics-core` | Hosts the compendium-style collection. Holds only the listing of coin toss as one entry under the bonus mark | padparadscha-table |
| `classics/desktop` | Client that produces a bevy window. Runs the game server as another thread of the same process | padparadscha-classics-core, padparadscha-common, bevy (the window and render plugins, `bevy_winit` and friends) |
| `classics/headless` | The windowless game-server binary itself. Same commands and API as desktop | padparadscha-classics-core (without the window and render plugins) |
| `classics/mobile` | iOS / Android client. The same flow, with only the window changed | padparadscha-classics-core, padparadscha-common, bevy |
| `classics/web` | wasm client. **Does not bundle the server.** Uses only the same-origin server's API | padparadscha-classics-core, padparadscha-common, bevy (the web backend), wasm-bindgen / web-sys |

- Layout matches asteria: libraries in `crates/` and `<artifact>/crates/`, binaries in short-named directories at the repository root (package names carry the `padparadscha-` prefix).
- classics-core depends on padparadscha-table alone. padparadscha-voxel is not used in this milestone.
- The app crates do not depend on eframe. egui is needed only for the chat panel, title and settings widgets. corundum-framework-gui is added with its `eframe` feature off (which is what sapphire-agent-gui already does).
- What classics/web reads out of padparadscha-table is only the body and table rendering and the resting-pose read-out. The physics simulation runs on every peer (§5.2).

### 2.2 Direction of Dependencies

- `padparadscha-common` uses `corundum-framework-platform` (dirs, locale) and `corundum-framework-gui` (look, theme, i18n, icons).
- `padparadscha-table` depends on `padparadscha-common` and on rapier3d.
- `padparadscha-classics-core` depends on `padparadscha-table` alone.
- The four clients depend on classics-core and on padparadscha-common. They do not depend on one another.
- **The app is an app with bevy plugins added, and common and table are among those plugins** (the shape of sapphire-agent-gui's `AgentGuiPlugin`). Everything about windows and per-OS plumbing stays on the app side only.
- **exactly one egui**. Every crate uses the same major of egui as the one bevy_egui re-exports (two egui stacks cannot share one app).
- OS-specific handling goes into `#[cfg]` modules. Directories are taken only through `corundum_framework::dirs`.

### 2.3 Core Technologies

| Purpose | What is adopted |
|---|---|
| The window and the runtime axis | **bevy 0.20** (added with `default-features = false`; the app adds the window and the backend). headless merely leaves out the window and render plugins |
| The egui widgets (chat panel, title, settings) | bevy_egui (the sapphire-turtle fork, the egui 0.37 branch) plus egui_extras (`svg`; turns Lucide icons into textures) |
| look and translation | corundum-framework-gui (`look`, `theme`, `i18n`'s `Resources` / `register` / `install`, `check`). Each crate embeds its own `.ftl` and `register`s it |
| OS / smartphone directories and plumbing | corundum-framework-platform (`dirs`, `locale`) |
| Binding the conversation with the agent | ACP (`agent-client-protocol`, WebSocket) |
| Letting the agent touch the game | MCP over ACP (`"type": "acp"`) |
| client ↔ game server | JSON-RPC 2.0 over HTTP, plus the WebSocket feed |
| runtime | tokio (the app client keeps one runtime of its own, apart from bevy's own scheduling. The shape of sapphire-agent-gui) |
| physics | rapier3d (rigid, `f64`) |
| RNG and read-out | `rand_xoshiro` (integers only; §7.1) and the `fixed` crate |

## 3. One Session (Chat and Game Are the Same Room)

- The game server is the **ACP client**; sapphire-agent is the **ACP server**. They bind **exactly one** session.
- The user's utterances and the game's state changes both arrive at the agent as the same conversation. "<user> tossed the coin. It came up heads." lines up in the conversation as one utterance.
- The agent's replies appear in the chat panel alongside the game.
- The game server is also an MCP server, and publishes its own tools over the ACP connection (§4). The agent uses those to interfere with the game; the user interferes from the client. **The two are peers.**

### 3.1 Game-Originated Lines: Canonical in the Log, Rendered by the Viewer

| thing | canonical form | what lands in the conversation (the ACP session) | what lands on the client feed |
|---|---|---|---|
| the user's utterance | text | text | text |
| the agent's reply | text | text | text |
| a game-originated state change | **one `seq`-tagged log row** (`id` and `args`) | structured as-is (§3.2) | structured as-is |

- Switching language later does not change the old rows in the log. That is why the only thing that changes is the viewer's rendering.

### 3.2 The Language of a Line in the Conversation

- **What the game server carries is a structured line, nothing else.** Turning it into text is the reader's job: ① the session (the agent) side renders with the session's language setting, ② the client renders in the viewer's own language (`tr!`). Two clients may read one session in different languages (desktop in Japanese, the browser in English).
- Whether it is a command-log-originated event (§7.3) or the `GameEvent` of §6.5, when it rides onto ACP's `session/update` the wording follows ①, that is, **the session's (the agent's) language setting**. ② (the window's chat panel) renders in the viewer's language. These two are kept distinct.

## 4. padparadscha-common (the Overall Flow)

Shipped as one bevy plugin. The app's app adds bevy's `DefaultPlugins` and its own window, then adds this plugin and the table plugin on top (the shape of sapphire-agent-gui).

### 4.1 Startup and Server Connection

1. Take the data, cache and settings directories from `corundum_framework::dirs`.
2. Read the app's settings: `[acp].url` (the agent's URL; local by default), `[server].mode` (`local` / `remote`). Language and theme are already in corundum's `look`, so nothing is carried here.
3. Under `local`, add the table's plugin to the same process (desktop, mobile). Under `remote`, connect to the URL. headless merely leaves out the window and render plugins; the set of plugins itself is the same. It is the server itself, so it carries no `[server].mode`.
4. Connect to `[acp].url` over WebSocket. If the API-version handshake does not match, refuse the connection and show a message that is clear.

### 4.2 Chat Panel

- One screen rendered in the egui that bevy_egui rides into the window. Not an egui shell (corundum-framework-gui's `Shell` / `ShellApp`). Usable at narrow widths (a phone, a browser corner).
- Top to bottom: utterances, replies, system-role lines (the game-originated ones). One input. An utterance reaches the game server as `chat.send` and becomes the ACP user message.
- The chat panel only renders the flow of the conversation and the log; it carries no game state of its own.

### 4.3 Registration as an MCP Server

- The game server is also an MCP server and publishes tools over the ACP connection. On the sapphire-agent side the tool name becomes `acp__<serverName>__<tool>`. The MCP server name this milestone publishes is `table`.
- Local or remote, client ↔ game server is JSON-RPC 2.0 (`POST /rpc`) and the feed (`GET /ws`); **only the route changes** (local is the `127.0.0.1` loopback). Unlike sapphire apps, no IPC.

## 5. padparadscha-table (the Tabletop Game Engine)

### 5.1 The Table and the Command Queue

- `Table`: the table (dimensions, edge, wall). Bodies are only what is placed on top of it.
- `Session`: carries the session ID, the **seed (§7.1)**, the **command queue**, and the list of bodies.
- **This milestone uses a plain FIFO queue.** Accepted commands enter the queue in order, and their outcome appears as one conversation line. Every row carries who issued it (`Actor`: user / agent).
- A command and the `GameEvent` it produces **share exactly one `seq`** (one log row per command; §6.5).
- **No strict alternating turns (whose turn it is, waiting, who is up).** Left as a later game-side concern (out of scope in §1).

### 5.2 Rigid Bodies and Physics

- A rigid body is handled as one `Body`. Body shapes are not rebuilt per game.
- Coin toss and the dice that come later are the same "body bouncing on a table". So the engine carries `simulate` alone. Physics is not rewritten per game.
- The physics types are rapier3d's own `Vector3<f64>`. No fixed-point types go into the simulation side. The only fixed point is the read-out of §7.1.
- **The simulation runs in every peer's process.** The game server is **the authority on the seed and the outcome** (the server decides the correct seq and the adjudication). Clients run the same thing on their own side from the same command log and seed, and build the same scene (§7.3).

### 5.3 State Change → `GameEvent` → Conversation Flow

- A state change becomes `GameEvent { id, args, seq }`. `id` is a Fluent message ID; `args` is `who` (who tossed it) and the outcome.
- The `GameEvent` goes to two places:
  1. **the conversation**: as one line of text rendered in the language of §3.2
  2. **the feed**: structured as-is to the client (re-rendered into the viewer's language)
- This conversion layer is what lives in padparadscha-table. It is not something rewritten per game.

## 6. Coin Toss (an Engine Capability, a Bonus)

### 6.1 The Body

A disc with thickness: two faces (heads, tails) and an edge. The mesh is made by the engine side. The dimensions are one coin (diameter 24.0mm, thickness 1.8mm, mass 5.0g; SI `f64` values).

### 6.2 Tossing

On receiving a `toss` command, the body is spawned on the table with the initial position, orientation, velocity and angular velocity that the seed decides.

### 6.3 Bouncing to Rest

rapier3d runs at a fixed 120Hz step, and when the body comes to rest a keyframe (position and orientation) is struck (§7.2).

### 6.4 Read-Out

Read the angle between **the normal of the face pointing up on the resting body** and the table's normal.

| angle | outcome |
|---|---|
| under 45° | that face. `heads` for heads, `tails` for tails |
| 45° or more (standing on its edge) | cannot be adjudicated, so **it re-tosses on its own**. If the third try also fails to decide, "standing on edge" is emitted as an event |

### 6.5 Re-Toss Numbering

**A re-toss does not create a new row.** One command leaves exactly one log row (§5.1). It is carried in the `attempt` of `args = { who, outcome, attempt }` (the re-toss number, starting at 0; 0 ≤ attempt ≤ 2). If even the third try fails to decide, `outcome = edge`.
- Each re-toss also draws **only from that one command's own stream of randomness (the one stream pulled from `(seed, seq)`; §7.1)**. Since there is only one stream per command, the `attempt` number comes out deterministically the same across targets.

### 6.6 The Event

`coin-toss.outcome`, `args = { who, outcome, attempt }` → "<user> tossed the coin. It came up heads."

### 6.7 The classics-core Side's One Line

classics-core holds only the listing of "coin toss" in the collection under `kind = bonus`. It carries neither physics nor adjudication.

## 7. Sync: Deterministic Simulation and Input Commands

### 7.1 Seed and Randomness

- `Seed(u64)`: **one per session**. Made once when the session is created, and delivered to clients at join (§7.3).
- The randomness for one command comes **only from a stream of unsigned integers** pulled from `(the session's seed, the command's seq)`. What is named is **`Xoshiro256PlusPlus` from `rand_xoshiro`** (state is u64 × 4; `(seed, seq)` is expanded by SplitMix64 to seed it). Because `f64` arithmetic cannot be let into the randomness, no floating-point RNG generator is included.
- **How integers are mapped to fixed point**: the upper 32 bits of the `u64` go straight into the fraction of `fixed`'s `U0F32` (0 integer bits, 32 fraction bits). The `[0, 1)` fixed-point value comes out as-is, with no truncation and no rounding. A range `[lo, hi)` is decided by the fixed-point multiply `lo + (hi - lo) * x` alone.
- The angle comparison of §6.4 is done with an `I16F16` dot product alone (the `cos` is a dot product taken after the two orientations being compared are dropped onto the same fixed point). `cos` and `atan2` stay out of the adjudication so that it is not swayed by a target's `f64` arithmetic.
- The values handed to the body (initial velocity, angular velocity, initial orientation) come from those fixed-point values.

### 7.2 Commands (What the Sync Carries)

Game state is synced by **deterministic simulation and input commands**. Not a stream of every frame.

| command | content | result |
|---|---|---|
| `toss` | who tossed it | the §6 outcome, as one `GameEvent` log row |
| `log` | a note the user typed (the name of a body, and the reason) | rides into the conversation as one line. Physics does not move |

- There is exactly one condition for calling rest: "the body came to rest". Its keyframe (position and orientation) is carried as the command's result.
- The keyframe is **the authoritative outcome reference**. Every peer keeps running the simulation on its own side (§5.2), so the keyframe is used **only as the fallback** for when a peer's own queue has not caught up — not instead of the simulation. (If §13.1's prototype shows determinism does not hold, §7 is rewritten as a whole.)
- The user issues the same `toss` from the client's "toss" button; the agent issues it from `acp__table__toss_coin`. The ordering is held by the queue of §5.1.

### 7.3 The Log: Snapshot and Feed Continue Without a Gap

- The log is **append-only and `seq`-tagged**. A command and the `GameEvent` it produces share exactly one `seq` (§5.1, §6.5).
- A client arriving is given, at join, **a snapshot of the log up to some seq**.
- The feed flows **from the continuation of that seq**. **There being no gap between snapshot and feed (gapless replay) is the contract**, and is a target of §12's tests (§9, §12).
- A connected client runs the same simulation on its own side and builds the same scene (§5.2).
- **The queue does not vanish while disconnected**: once received, the client remembers its `seq`. On reconnect it passes **`from_seq`** (the next `seq` it has not received yet) to `session.join`, and receives only what was missing. `from_seq` travels as an argument of `POST /rpc`'s `session.join` alone; the `GET /ws` handshake carries `ticket` and nothing else (§9.1).

## 8. The Same API Across the Four Targets

| target | game server | what it uses |
|---|---|---|
| desktop | embedded, as another thread of the same process | `POST /rpc` and `GET /ws` (loopback) |
| mobile | embedded, as another thread of the same process | same as above |
| headless | **the server binary itself** | same as above (visible to remote clients) |
| web (wasm) | **not bundled** | the same-origin server's `POST /rpc` and `GET /ws` |

- A server started locally publishes the same API as the server on the origin the browser loaded the page from. Which is why it can be added as an easter egg to poke at in a corner of the site.
- The distribution shape also matches asteria: one command; desktop and mobile add a window to that command; headless is the same binary without a window.
- wasm's `GET /ws` uses the browser's `WebSocket`.
- The mobile (iOS) window runs on bevy's iOS window path (what sapphire apps already carries). It comes last of the four (§14).

## 9. API

| route | purpose |
|---|---|
| `POST /rpc` | JSON-RPC 2.0. `server.info` (handshake: API version), `session.join` (**snapshot: returns the session ID, the seed and the log up to a `seq`. Takes `from_seq` as an argument when reconnecting, §7.3**), `session.ticket` (§9.1), `game.command` (issues `toss` / `log`), `chat.send` |
| `GET /ws?ticket=…` | the feed. From the seq continuing the snapshot, streams log appends and conversation content (utterances, replies, the structured form of `GameEvent`s) |

### 9.1 `GET /ws` Auth (a Browser Cannot Carry `Authorization`)

- What cannot carry `Authorization` is only the browser's `WebSocket` handshake (unlike fetch, the headers cannot be chosen). So **`GET /ws` is connected with `?ticket=`**. Local and mobile use the same shape (only the route changes; §4.3).
- The ticket comes from `session.ticket` on `POST /rpc`, and is **single-use and short-lived** (30 seconds by default, usable once). The `POST /rpc` side can carry its own header (local, mobile or browser, any of them `Authorization: Bearer <token>`; the browser takes the token from the page), so **handing that auth to the WebSocket handshake once** is enough.
- Tickets are bound to a session ID. Unknown, expired, or a second use gives 401 at the handshake. Reconnecting is done by re-issuing with a new ticket (§10).

## 10. Error Handling

| situation | handling |
|---|---|
| cannot bind the agent, or it dropped mid-way | the screen keeps running as "no agent". The game stays playable. Reconnection happens, and input is still accepted meanwhile |
| a gap appeared between the feed and the snapshot | the §7.3 contract. The client states the next seq it already has, and receives only what was missing |
| ticket unknown, expired, or used a second time | 401 at the handshake. The screen fetches a new ticket and reconnects |
| API versions do not match | show a clear message and refuse the connection |
| still standing on its edge after 3 tosses | `outcome = edge` is emitted as an event (§6.5). **The queue itself is not blocked**: other peers' commands pass through as-is, and queued commands stay queued. What is decided is only "the user tossed once more and an outcome came out" |
| the next command arrived while a simulation is still running | the command enters the queue and waits its turn (§5.1) |
| wasm cannot bind the same-origin server | one line appears: "server not found". Requests can be retried |
| a translation id is missing | the id is emitted as-is, and a miss is logged once (corundum's `i18n` behavior) |

## 11. Success Criteria

This milestone is **a probe of whether the "playing together" feeling is reachable at all**. So the criteria are the four below, and **what is left at the end is what the author felt**.

1. **desktop**: the user talks in the chat panel and tosses the coin in the game window. "<user> tossed the coin. It came up heads." appears in the chat as a system message.
2. **sapphire-agent**: binds an ACP session and replies into that flow (it can also answer via `acp__table__toss_coin`).
3. **wasm**: the browser binds the same-origin server and re-enacts the same experience.
4. **adjudication is a function of the physics**: heads and tails are decided by the resting-pose read-out, and the determinism tests pass.

If the feeling that came out does not amount to "playing together while chatting", then that shortfall is reported as this milestone's deliverable. A result of "no good feeling came out" is still a pass for this milestone.

## 12. Tests

- **determinism**: from the same seed and the same log, **the same adjudication** comes out on x86_64, aarch64 and `wasm32-unknown-unknown`. For the body's resting position and orientation, check **agreement at the read-out granularity (the digits §6.4's angle adjudication needs)**, not down to the low bits of `f64`, and do not treat low-bit differences that leave the adjudication unchanged as a mismatch. Separately confirm that §7.1's integer stream is the same across targets. Include that each re-toss draws from the same stream, so `attempt` is the same everywhere (§6.5).
- **read-out**: build a set of resting poses and confirm that the three cases — heads, tails, standing — split exactly as §6.4 says. Confirm that something standing on its edge always re-tosses, and that §6.5's `attempt` is filled. Confirm that failing to decide by the third try sets `outcome = edge`, and that the FIFO queue is not blocked meanwhile (§10).
- **gapless replay (the §7.3 contract)**: the `seq` of the join snapshot and the `seq` of the feed's first row continue one from the other; a client that dropped and reconnected passes `from_seq` and receives each of the same rows exactly once. Also check that one command leaves exactly one log row (the command and its `GameEvent` share one `seq`; §5.1).
- **conversation**: the `GameEvent` rides into the session as one line, rendered in the session's language (§3.2). That it is re-rendered from `args` into the viewer's language.
- **ticket**: `session.ticket`'s ticket is usable exactly once; expiry and a second use give 401; a new ticket lets it be re-issued.
- **end to end**: start headless, and confirm `session.join` → `game.command` → one conversation line → another client joins and gets the same line. For the agent side, include both the integration test that starts sapphire-agent and a small fake ACP server in its place.
- **wasm**: in CI, use `wasm-bindgen-test` to confirm that binding the same-origin server carries the same experience through. The wasm builds of bevy, bevy_egui and rapier3d are carried through a CI job too.
- **CI**: build, test and `cargo deny` on Linux (x86_64 and arm64), macOS, Windows. The wasm build and `wasm-bindgen-test` are Linux jobs only.

## 13. Risks and Early Verification

At an early point of the implementation plan, confirm the following by prototype. **All four of these are technical risks of milestone 1 itself**, and none is carried over into a later milestone.

1. **rapier3d does not promise determinism** (`f64`, SIMD, aarch64's FMA, wasm's floating point). Confirm how far agreement holds at §12's adjudication granularity. If it does not hold, drop to the shape of **the server carrying the adjudication and the resting keyframe, and clients only interpolating** (the shape of syncing by command and seed is unchanged). Then §7 is rewritten.
2. **bevy + bevy_egui build under wasm32-unknown-unknown**. bevy's web backend (`bevy_in_wgpu`, and WebGL2 through `wgpu-surf`), bevy_egui, `wgpu`'s browser backend. The desktop side already goes through in sapphire-agent-gui, so follow that shape.
3. **a table and bodies can be rendered on that same bevy** (including both headless and web).
4. **MCP over ACP runs on the sapphire-agent side** (the 2026-11-10 spec is in production).

## 14. Order of Work (Outline)

The details are decided in the implementation plan. Keep a shape that lets us return to the other targets while desktop is being advanced.

1. Workspace foundation: licenses, CI, `cargo deny`, incorporating the corundum and bevy crates.
2. The §13.1 and §13.2 prototypes (physics determinism; bevy + bevy_egui + rapier3d wasm).
3. `padparadscha-common`: dirs; the bevy plugin; look, language and theme; the ACP session; the chat panel; MCP registration.
4. `padparadscha-table`: `Table`, the command queue (§5.1's FIFO) and `GameEvent` and `seq`, rapier3d, `toss` and `log`.
5. Coin toss's body and read-out, `coin-toss.outcome`.
6. `padparadscha-classics-core`: listed as one bonus entry.
7. `classics/headless` (the server binary) and `classics/desktop` (produces the bevy window, with the game server built in).
8. `classics/web`: bind the same-origin server and re-enact the same experience.
9. The end-to-end tests with sapphire-agent bound, and the §11 checks.
10. mobile (iOS). OS-specific handling goes into `#[cfg]` modules, keeping the §2.2 shape while desktop is advanced. The iOS window runs on bevy's iOS window path. corundum-framework-platform then carries the OS plumbing (dirs, locale, keyboard) on top of that. Which is why it is placed last of this milestone's four.
