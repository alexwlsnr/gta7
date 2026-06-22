# Incompetech Music Acquisition Guide

This is the exact pattern we used to add the Kevin MacLeod / Incompetech music that now ships in `assets/music/`.

## What we have now

The game currently embeds MP3 files directly into the binary from `assets/music/` via `include_bytes!` in `src/sound.rs`.

Current groupings:

- **Radio tracks**: `GoCart.mp3`, `LaserGroove.mp3`, `RetroFutureClean.mp3`, `RetroFutureDirty.mp3`, `FunkGameLoop.mp3`, `SpaceFighterLoop.mp3`, `Loopster.mp3`, `RocketPower.mp3`, `SonOfARocket.mp3`, `HappyHappyGameShow.mp3`
- **Walk tracks**: `BassaIslandGameLoop.mp3`, `TownieLoop.mp3`
- **Wanted/chase tracks**: `ZombieChase.mp3`, `ChasePulse.mp3`

See `src/sound.rs:69-119`.

## Source and license

Source catalog:
- `https://incompetech.com/music/royalty-free/pieces.json`

Direct MP3 base URL:
- `https://incompetech.com/music/royalty-free/mp3-royaltyfree/`

License used for the current bundle:
- **CC BY 4.0**
- Attribution file in repo: `assets/music/ATTRIBUTION.txt`

Important: keep the attribution file up to date with the **original track titles and artist credit**, even if we rename the local files.

## How we found tracks

We did not guess direct URLs one by one. We used the catalog JSON first.

Each catalog entry includes a `title` and a `filename`. The `filename` is the key field because the direct MP3 URL is:

```text
https://incompetech.com/music/royalty-free/mp3-royaltyfree/<url-encoded filename>
```

Example:

- Catalog title: `Go Cart - Loop Mix`
- Catalog filename: `Go Cart - Loop Mix.mp3`
- Direct download URL:

```text
https://incompetech.com/music/royalty-free/mp3-royaltyfree/Go%20Cart%20-%20Loop%20Mix.mp3
```

## Recommended workflow for adding more music

### 1. Search the catalog for candidates

Use the catalog JSON and filter by keywords that match the vibe you want.

Example Python one-liner:

```bash
python3 - <<'PY'
import json, urllib.request
url = 'https://incompetech.com/music/royalty-free/pieces.json'
data = json.load(urllib.request.urlopen(url))
keywords = ['drive', 'chase', 'loop', 'funk', 'city', 'game', 'retro']
for item in data:
    title = item.get('title', '').strip()
    filename = item.get('filename', '')
    hay = f"{title} {filename}".lower()
    if any(k in hay for k in keywords):
        print(f"{title} -> {filename}")
PY
```

This is how we surfaced tracks in the same family as the current bundle.

### 2. Pick tracks by gameplay role

Keep the buckets intentional:

- **Radio**: upbeat driving / cruising / arcade energy
- **Walk**: lower-energy town ambience
- **Wanted**: chase / action / pressure

That structure already exists in `src/sound.rs`, so new tracks should fit one of those buckets instead of introducing ad-hoc playback rules.

### 3. Download the MP3 from the catalog filename

Prefer Python stdlib here so the filename is URL-encoded correctly.

Example:

```bash
python3 - <<'PY'
from pathlib import Path
from urllib.parse import quote
from urllib.request import urlretrieve

base = 'https://incompetech.com/music/royalty-free/mp3-royaltyfree/'
source_filename = 'Go Cart - Loop Mix.mp3'
local_filename = 'GoCart.mp3'

dst = Path('assets/music')
dst.mkdir(parents=True, exist_ok=True)
urlretrieve(base + quote(source_filename), dst / local_filename)
print(dst / local_filename)
PY
```

Notes:
- `source_filename` is the exact `filename` field from `pieces.json`
- `local_filename` is our shorter repo-friendly name
- Renaming locally is fine as long as attribution stays accurate

### 4. Update attribution

Add the track to `assets/music/ATTRIBUTION.txt`.

Use the original title, not the shortened local filename.

Example format:

```text
Music tracks by Kevin MacLeod (incompetech.com)
Licensed under Creative Commons: By Attribution 4.0 License
http://creativecommons.org/licenses/by/4.0/

Tracks:
- "Go Cart - Loop Mix" Kevin MacLeod (incompetech.com)
- "Laser Groove" Kevin MacLeod (incompetech.com)
```

If we add more tracks, append them here.

### 5. Wire the file into the game

Add the MP3 to the right array in `src/sound.rs`.

Examples already in the code:

```rust
let radio_files = &[
    include_bytes!("../assets/music/GoCart.mp3").as_slice(),
    include_bytes!("../assets/music/LaserGroove.mp3").as_slice(),
];

let walk_files = &[
    include_bytes!("../assets/music/BassaIslandGameLoop.mp3").as_slice(),
    include_bytes!("../assets/music/TownieLoop.mp3").as_slice(),
];

let wanted_files = &[
    include_bytes!("../assets/music/ZombieChase.mp3").as_slice(),
    include_bytes!("../assets/music/ChasePulse.mp3").as_slice(),
];
```

The loader already does the rest:
- `audio.new_music_from_memory(".mp3", bytes)`
- `set_looping(true)`
- `set_volume(0.3)`

## Verification checklist

After adding or replacing tracks:

1. `cargo build`
2. `cargo test`
3. Run the game
4. Verify:
   - radio plays
   - walk/wanted mode switching still works if touched
   - no MP3 decode failures in startup logs
   - pause-menu music volume slider still affects playback

## Practical notes

- The files are embedded with `include_bytes!`, so every added MP3 increases binary size.
- Keep filenames short and stable in-repo, but preserve original names in attribution.
- Prefer tracks that loop cleanly or tolerate looping well, because playback uses `set_looping(true)`.
- The current repo already has more tracks than the original first pass, so if you add more, keep the buckets curated rather than just dumping in everything that matches a keyword.

## Minimal repeatable recipe

1. Query `pieces.json`
2. Find a candidate track's `filename`
3. Download from `mp3-royaltyfree/<url-encoded filename>` into `assets/music/`
4. Add credit to `assets/music/ATTRIBUTION.txt`
5. Add `include_bytes!` entry in the correct bucket in `src/sound.rs`
6. Build, test, run
