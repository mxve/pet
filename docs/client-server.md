# server / client

## idea
- server = game, runs all progression
- client = frontend
- client only sends what to do, server decides how

## comms
- quake style udp
  - `\xFF\xFF\xFF\xFF`, version byte, kind byte, body
- body = `postcard`
- max 1200 bytes
- client asks, server just replies
- bad/unknown header -> drop

## keys
- server: one x25519 keypair
  - priv stays on server
  - pub baked into client at build
- signup: client makes keypair
  - both combine -> shared secret
  - secret = sha256("pet account v1" + shared)
- client priv in `~/.pet/account.toml`
- server keeps client pub
- lost account file = lost pet

## signing
- every req + reply ends w/ hmac-sha256(secret, packet)
- server checks tag first, before anything else
- client checks replies too -> nobody fakes the server

## challenge
- client asks for challenge, server answers hmac(ip + port + minute)
- every req carries it -> faked sender addr = dropped
- `GetChallenge` padded to answer size

## seq numbers
- every req has rising nr (client clock ms)
- lower/same -> replay -> dropped
- clock went back -> server says `Stale { last }`, client continues from there

## retries
- cmds sent 2x right away (2nd after 30ms), then every 250ms up to 2s
- copies = exact same bytes
- server keeps last 16 replies per account
- exact retry -> same reply again, nothing applied twice, doesnt count vs limit

## requests
- `Sync { since, after }` -> what changed + new news, 1 packet
- `Command { command }` -> `Done` or `Refused { reason }`, both w/ snapshot
- `Duel` -> battle or `Waiting`
- `Rekey { public }` -> swap acc key
- `Leaderboard { board, page }` -> rows + own rank

## commands
- wishes not results -> "feed", never "food = 90"
- act (feed/pet/play), sleep, wake, train, rest, focus, buy, feed food, theme, flee
- server: catch up -> apply w/ `core` -> save -> reply
- `busy_until` per action -> no spamming faster than a human

## threads
- std threads + channels, no async
- 1 receiver: reads socket, sends packet to worker by acc id (ip if no acc yet)
- workers: 1 per core, max 8, own sqlite conn each
- same acc -> same worker always -> in order, no locks
- 1 chores thread: challenge secret, bucket cleanup, duel fallback, news pruning, backup
- shared stuff (challenge secret, ip limits, duel queue) behind mutex
- battles touch 2 accs -> write only if both revs unchanged, else redo
- workers remember latest rev + news id -> idle syncs never hit db

## sync
- every 250ms 1 `Sync { since, after }` -> "did my pet change, did anything happen"
- ~4 pkts/s per player

## state
- server stores 1 world per acc + rev nr
- time passing != change, server catches up when it touches a world
- snapshot = world + rev + server time
- client draws snapshot, moves bars forward locally so its smooth, throws that away on next snapshot
- older rev than client has -> ignore
- server never trusts client time

## news
- lvl ups (also while away), unlocks, achievements, daily bonus, duels, fight results
- come w/ every `Sync`, become notices + cheers
- more news than fits -> sync again right away
- on start -> everything since last session

## battles
- server picks seed, runs whole fight, pays both sides, then sends `{ seed, fighters, start_at }`
- both clients replay from seed at same server time
- flee -> server settles at that swing
- client replay changes nothing, result already saved

## limits
- by identity = by acc, counted only AFTER sig check
  - so nobody burns someone elses budget by faking acc id
  - 10 req/s, burst 20, per acc
  - same acc on 2 pcs = 1 budget, 2 accs on 1 wifi = separate
- by ip only where theres no identity yet (`GetInfo`, `GetChallenge`, `Register`)
  - 10 challenges/s, 5 signups/h, 50 pkts/s total
- cmds also limited by game itself (`busy_until`)
- over -> `Slow { retry_after }`
- token buckets in mem, no ips on disk

## errors
- `Refused` -> rose notice
- `Slow` -> wait
- `Stale` -> nothing, just continue
- `Outdated` -> "please update"
- `Unknown` -> account screen
- no answer 2s -> "offline, retrying", keep syncing every 2s

## storage
- sqlite -> `rusqlite`
- on open:
  - `journal_mode = WAL`
  - `synchronous = NORMAL`
  - `foreign_keys = ON`, `busy_timeout = 5000`
- tables: accounts, worlds, replies, news, battles, meta
- indexes on news + replies (hit on every poll/retry)
- worlds + events as toml text
- backups via sqlite tooling

## misc
- `scripts/keygen.sh` once -> `server.key` + `server.pub` (refuses to overwrite)
- `pet-server --key ... --db ...`
- udp 27960
- `server.pub` -> gitea action secret `PET_SERVER_KEY`
- release workflow builds w/ `PET_SERVER_KEY` + `PET_SERVER`, baked in at compile time

## leaderboard
- boards: level, each skill, combat lvl, wins
- shows rank, pet name, species, number. no ids no ips
- always see own rank
- `worlds` keeps `total_xp`, `combat`, `wins` cols (indexed), updated on every change
- 1 `ORDER BY ... LIMIT` per board
- ~20 rows per datagram, paged
- numbers all come from server anyway -> nothing extra to check

## dev mode
- debug builds only, both sides, code cut from release w/ cfg flags
- server: `pet-server --dev` -> accepts `Cheat` cmds, says `dev: true` in `Info`
- client: `pet --dev` -> normal keys + signed pkts, overlay, ctrl cheat keys
- client refuses to start if server isnt a dev server
- dev server has own keypair, dev client built w/ `PET_SERVER_KEY`
- cheats = cmds, applied on server: xp, next lvl, drain/fill stats, speed
- speed = dev server clock (1 / 10 / 60 / 600 / 3600x)
- `Cheat` = last `Command` variant -> other cmds encode same in debug + release