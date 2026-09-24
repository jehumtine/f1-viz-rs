# f1-data

A pure-Rust client and replay engine for the F1 LiveTiming archive
(`livetiming.formula1.com/static`) — the same raw feed that powers FastF1,
OpenF1 and F1's own timing app. Fetch a finished session, cache it locally,
and query the exact state of every car at any millisecond of the session.

- **Archive transport only** (plain HTTPS, no auth, no websocket) — every
  session that has already finished is available.
- **Two-tier disk cache** — raw feed text on first fetch, bincode-serialized
  parsed structs afterwards. Second and later runs load a full race in tens
  of milliseconds.
- **Unified timeline** — positions, car telemetry, track status, weather,
  lap count and race control messages folded into one ordered event stream
  keyed by a single time type.
- **On-demand interpolation** — cars are sampled between irregular samples
  (~4 Hz position, ~10 Hz car data) with binary search + linear lerp;
  discrete channels (gear, DRS, brake) are held, never interpolated.
- **Scrubbable playback** — `frame_at(t)` works at any rate, in any order;
  rewinds reset the discrete-state reducer automatically.

## Architecture

```
src/
├── lib.rs            public API boundary + re-exports
├── bin/demo.rs       test harness / worked example (not part of the lib)
├── error.rs          F1Error (thiserror)
├── api/              ── the wire layer ─────────────────────────────
│   ├── client.rs     F1ArchiveClient: HTTP fetch, stream splitting, caching
│   └── decode.rs     base64 + raw-deflate decoding for *.z feeds
├── model/            ── the domain layer ───────────────────────────
│   ├── raw.rs        serde targets for F1's messy JSON (internal)
│   ├── domain.rs     clean typed structs handed to callers
│   └── time.rs       RawOffset — the canonical timeline key
└── engine/           ── the logic layer ────────────────────────────
    ├── clock.rs      SessionClock: offset ↔ wall-clock UTC
    ├── timeline.rs   TimelineEvent, Timeline, SessionState (reducer)
    ├── track.rs      CarTrack: per-driver interpolation
    └── player.rs     SessionPlayer + Frame: the playback cursor
```

The library has no side effects beyond its cache directory. Consumers
(a replay UI, an exporter, a TUI) depend on it as a path dependency and
drive everything through `SessionPlayer`.

## Quickstart

```toml
# In your consumer crate
[dependencies]
f1-data = { path = "../f1-data" }
tokio = { version = "1", features = ["full"] }
anyhow = "1"          # binaries: anyhow; the lib itself uses thiserror
```

```rust
use std::collections::HashMap;
use std::time::Duration;

use f1_data::api::client::F1ArchiveClient;
use f1_data::engine::clock::SessionClock;
use f1_data::engine::player::SessionPlayer;
use f1_data::engine::timeline::Timeline;
use f1_data::engine::track::CarTrack;
use f1_data::model::RawOffset;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Point at an archived session.
    //    Path format: /{year}/{date}_{meeting}/{date}_{session}/
    let client = F1ArchiveClient::new(
        "/2023/2023-05-07_Miami_Grand_Prix/2023-05-07_Race/".to_string(),
    );

    // 2. Fetch feeds. First run downloads (~seconds); later runs hit cache.
    let drivers  = client.get_driver_list().await?;
    let positions = client.get_position_data().await?;
    let car_data  = client.get_car_data().await?;
    let status    = client.get_track_status().await?;
    let weather   = client.get_weather_data().await?;
    let messages  = client.get_race_control_messages().await?;
    let laps      = client.get_lap_count().await?;

    // 3. Calibrate wall-clock time (optional, for display only).
    let clock = SessionClock::from_positions(&positions);

    // 4. One interpolation track per driver.
    let mut tracks = HashMap::new();
    for &num in drivers.keys() {
        tracks.insert(num, CarTrack::build(num, &positions, &car_data));
    }

    // 5. Fold every feed into one sorted timeline, then play it.
    let timeline = Timeline::from_feeds(
        positions, car_data, status, weather, messages, laps,
    );
    let mut player = SessionPlayer::new(timeline, tracks);

    // 6. Fixed-rate replay loop (25 fps here; any rate works).
    let mut t = Duration::ZERO;
    while t <= player.duration().0 {
        let frame = player.frame_at(RawOffset(t));
        // frame.state  -> flags, lap count, weather, last race control msg
        // frame.cars   -> HashMap<u8, UnifiedCarState> (position + telemetry)
        t += Duration::from_millis(40);
    }

    Ok(())
}
```

## Core concepts

### Time: `RawOffset`

Every `.jsonStream` line is prefixed with a 12-character timestamp
(`HH:MM:SS.mmm`). That prefix — parsed into `RawOffset(Duration)` — is the
**canonical ordering key** for the whole library: it is the one notion of
time every feed has, and it is monotonic within a session file.

Only `Position.z` and `CarData.z` additionally embed absolute UTC
timestamps. `SessionClock` learns `t0` (the UTC instant at offset zero)
from the first usable position sample and converts offsets to wall-clock
time on demand:

```rust
let clock = SessionClock::from_positions(&positions);
let wall = clock.to_absolute(RawOffset(Duration::from_secs(3600)));
// Some(2023-05-07T19:31:00Z)-ish
```

### Two kinds of state

- **Discrete state** (flags, lap count, weather, race control messages)
  behaves as *last value wins*. `SessionState` is a reducer folded over the
  timeline by the player's cursor. Never interpolate a flag.
- **Continuous state** (X/Y/Z, speed, RPM, throttle) behaves as a *signal*.
  Each driver gets a `CarTrack` of sorted samples; `interpolate_at(t)`
  binary-searches the bracketing pair and lerps. Discrete channels inside
  telemetry (gear, DRS, brake) are held at their last known value.

### `SessionPlayer` and rewinds

`frame_at(t)` is the single entry point for consumers:

- **Forward** calls are cheap — the cursor resumes where it left off.
- **Backward** calls reset the reducer and re-fold from zero (still
  sub-millisecond over a full race), so scrubbing a replay timeline is safe.
- Cars with no sample at or before `t` (e.g. still in the garage) are
  simply absent from `frame.cars`.

```rust
let frame = player.frame_at(RawOffset(Duration::from_secs(3600)));
println!("lap {}/{}", frame.state.lap.0, frame.state.lap.1);
for (num, car) in &frame.cars {
    println!(
        "#{num:>2} ({:>7.1}, {:>7.1}) {:>3} km/h gear {} drs {}",
        car.position.x_m, car.position.y_m,
        car.telemetry.speed_kph, car.telemetry.gear, car.telemetry.drs,
    );
}
```

### Raw event log

If you want every event rather than a resampled grid (e.g. exporting flag
changes to CSV), iterate the timeline *before* handing it to the player:

```rust
use f1_data::engine::timeline::TimelineEvent;

for event in timeline.events() {
    if let TimelineEvent::TrackStatus { at, event } = event {
        println!("{at}  status {} ({})", event.status, event.message);
    }
}
```

## Feeds and getters

| Method | Returns | Notes |
| --- | --- | --- |
| `get_session_data()` | `SessionInfo` | meeting/country/name/start date |
| `get_driver_list()` | `HashMap<u8, Driver>` | number → code, name, team, colour |
| `get_position_data()` | `Vec<PositionSample>` | X/Y/Z in metres, `on_track` flag; bincode-cached |
| `get_car_data()` | `Vec<CarData>` | rpm, speed, gear, throttle, brake, DRS; bincode-cached |
| `get_track_status()` | `Vec<(RawOffset, TrackStatusEvent)>` | codes: 1 clear, 2 yellow, 4 SC, 5 red, 6 VSC, 7 VSC ending |
| `get_lap_count()` | `Vec<(RawOffset, u32, Option<u32>)>` | (current, total); total kept as last known when F1 omits it |
| `get_weather_data()` | `Vec<(RawOffset, WeatherSample)>` | ~1 sample/minute |
| `get_race_control_messages()` | `Vec<(RawOffset, RaceControlMessage)>` | flags, penalties, investigations |

Driver keys are **racing numbers** (`u8`), matching `Position.z`/`CarData.z`.
Use `get_driver_list()` to map numbers → three-letter codes, names, teams.

## Caching

Two tiers, both under the OS cache directory
(`~/.cache/f1-livetiming/…` on Linux, via `directories::ProjectDirs`),
in one folder per session path:

1. **Raw tier** — every fetched feed file is stored verbatim. Finished
   sessions never change, so there is no invalidation logic.
2. **Bincode tier** — `positions.bin` / `car_data.bin` store the *parsed*
   structs, skipping base64 + deflate + JSON parsing on warm runs.

Delete the session's cache folder to force a re-fetch. If you change the
shape of any domain struct, delete the `.bin` files too — bincode is
layout-sensitive and will error on mismatch (surfaced as
`F1Error::Bincode`).

## Errors

All fallible calls return `Result<_, F1Error>`:

| Variant | Meaning |
| --- | --- |
| `Http` | reqwest failure (network, status) |
| `Json` | serde parse failure on a feed payload |
| `Base64` / `Decompress` | `*.z` payload decoding failure |
| `TimeParse` / `UnexpectedFormat` | malformed timestamps / offsets |
| `Io` | cache file read/write failure |
| `Bincode` | warm-cache (de)serialization failure |

Libraries match on this enum; binaries should just convert to `anyhow`
with `?` and move on.

## Performance

Measured on a full race session (~71 500 timeline events, ~36 000 position
samples, ~35 000 car-data samples):

| Stage | Time |
| --- | --- |
| Cold fetch (network) | seconds, once |
| Warm parse (raw cache → structs, rayon-parallel) | ~1–2 s |
| Warm load (bincode cache) | tens of ms |
| `frame_at(t)` (20 cars, discrete fold + interpolation) | ~27 µs |

At 27 µs/frame you have ~600× headroom against a 60 fps render budget.

## Data quirks (why `model::raw` looks the way it does)

The LiveTiming API is unofficial and inconsistent. The raw layer absorbs
all of this so the domain layer stays clean:

- Numeric-looking fields sometimes arrive as strings (`"1"` for track
  status, `"0"`/`"1"` for rainfall); raw structs parse as `String` and
  convert at the boundary.
- `LapCount` events occasionally omit `TotalLaps`; the reducer keeps the
  last known total instead of guessing.
- `RaceControlMessages` payloads wrap their items in a `Messages` array.
- `Position.z` entries are frequently incomplete (pit lane, off track);
  trust `on_track` before treating X/Y as a valid track point.
- Coordinates are track-local, in 1/10-metre units, converted to metres at
  parse time. There is no GPS reference.
- DRS channel: `0/1` off, `8` eligible/detected, `10/12/14` on.
- Session dates appear in several formats (with/without `Z`, date-only);
  parsing falls back leniently.

## Roadmap

- `discovery.rs`: walk `Index.json` (season → meeting → session) instead of
  hardcoding session paths.
- `TelemetrySource` trait so archive and a future live SignalR source are
  interchangeable at the call site.
- `TimingAppData.jsonStream` for tyre compounds/stints.
- Workspace `apps/` (replay UI, CSV exporter) consuming this lib by path.
