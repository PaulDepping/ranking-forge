# start.gg API — Project Notes

Reference for the start.gg GraphQL API as used by RankingForge. See `schema.graphql` in this directory for the full SDL schema. Run `fetch-schema.sh` to refresh it before extending the query set.

## API Basics

| Item | Value |
|---|---|
| Endpoint | `https://api.start.gg/gql/alpha` (stable production path despite the `/alpha` name) |
| Auth | `Authorization: Bearer <STARTGG_API_KEY>` header |
| Rate limit | 80 requests per 60 seconds |
| Complexity limit | 1000 nested objects per request |
| Protocol | GraphQL over HTTP POST, JSON body |

Authentication uses a shared server-side key (`STARTGG_API_KEY` env var). No OAuth is required for the read queries this project uses.

## Our 5 Operations

### 1. `search_games` — Find a videogame ID by name

**Purpose:** Resolve a human-readable game name (e.g. `"Super Smash Bros. Ultimate"`) to an internal `videogameId` used by all other queries.

**Query:**
```graphql
query($name: String) {
    videogames(query: { filter: { name: $name } }) {
        nodes { id name displayName }
    }
}
```

**Key fields:** `id` is the value stored as `game_id` in the `rankings` table and passed to `TOURNAMENTS_BY_USER_QUERY`. `displayName` may differ from `name`; we display `name`.

**No pagination needed:** The name filter is narrow enough that results fit in one page.

---

### 2. `user_by_slug` — Resolve a start.gg user slug to an internal ID

**Purpose:** Convert a user-supplied start.gg profile URL slug (e.g. `"user/abc123"`) to a numeric `userId` for subsequent queries.

**Query:**
```graphql
query($slug: String) {
    user(slug: $slug) { id player { gamerTag } }
}
```

**Key fields:** `id` is the numeric start.gg user ID stored as `startgg_id` on the `users` table. `player.gamerTag` is the display name.

---

### 3. `tournaments_by_user` — Paginate a user's tournament history

**Purpose:** Fetch all tournaments a user attended for a given game, with full event/phase/phaseGroup structure for import.

**Query:**
```graphql
query($userId: ID!, $gameId: ID!, $page: Int!, $perPage: Int!) {
    user(id: $userId) {
        tournaments(query: {
            page: $page
            perPage: $perPage
            filter: { videogameId: [$gameId] }
        }) {
            pageInfo { total totalPages }
            nodes {
                id name slug
                city addrState countryCode
                venueName venueAddress
                timezone isOnline numAttendees
                lat lng state
                startAt endAt
                events(filter: { videogameId: [$gameId] }) {
                    id name numEntrants startAt
                    slug state isOnline type
                    teamRosterSize { minPlayers maxPlayers }
                    phases {
                        id name bracketType phaseOrder
                        numSeeds groupCount state isExhibition
                        phaseGroups(query: { perPage: 100 }) {
                            nodes {
                                id displayIdentifier bracketType bracketUrl
                                numRounds startAt firstRoundTime state
                            }
                        }
                    }
                }
            }
        }
    }
}
```

**Pagination:** Loop over pages 1..`pageInfo.totalPages`. Use `perPage: 25` (safe within the 1000-object complexity budget given the deep nesting).

**Key fields:** `state` on Tournament/Event/Phase/PhaseGroup is the `ActivityState` field — see the quirks section below.

---

### 4. `event_entrants` — Paginate all entrants in an event

**Purpose:** Fetch every entrant (player or team) in an event so we can store seed numbers, final placements, and link back to start.gg user IDs.

**Query:**
```graphql
query($eventId: ID!, $page: Int!, $perPage: Int!) {
    event(id: $eventId) {
        entrants(query: { page: $page, perPage: $perPage }) {
            pageInfo { total totalPages }
            nodes {
                id initialSeedNum isDisqualified
                standing { placement }
                participants { gamerTag user { id } }
            }
        }
    }
}
```

**Pagination:** Loop over pages 1..`pageInfo.totalPages`. Use `perPage: 50`.

**Key fields:** `participants[].user.id` links the entrant to a start.gg user. `standing.placement` is the final finish position. `isDisqualified` marks DQ'd entrants separately from those filtered via score (see quirks).

---

### 5. `event_sets` — Paginate all completed sets in an event

**Purpose:** Fetch every match result for upset-factor computation.

**Query:**
```graphql
query($eventId: ID!, $page: Int!, $perPage: Int!) {
    event(id: $eventId) {
        sets(page: $page, perPage: $perPage, sortType: STANDARD) {
            pageInfo { total totalPages }
            nodes {
                id winnerId round fullRoundText totalGames
                completedAt vodUrl
                hasPlaceholder state identifier
                phaseGroup { id }
                slots {
                    entrant { id }
                    standing { stats { score { value } } }
                }
            }
        }
    }
}
```

**Pagination:** Loop over pages 1..`pageInfo.totalPages`. Use `perPage: 50`.

**Key fields:** `winnerId` is the entrant ID of the winner. `slots[].standing.stats.score.value` is the game count won by each player (or `-1` for a DQ — see quirks). `phaseGroup.id` links the set to its bracket.

## Known Quirks

### `ActivityState` type inconsistency

The API schema declares `ActivityState` as an enum, but the wire format differs by object type:

| Object type | Wire format | Example |
|---|---|---|
| `Event.state` | **String** | `"ACTIVE"` |
| `Phase.state` | **String** | `"COMPLETED"` |
| `Tournament.state` | **Integer** | `2` |
| `PhaseGroup.state` | **Integer** | `3` |
| `Set.state` | **Integer** | `3` |

The Rust deserialization handles this by declaring each struct's `state` field with its natural type: `EventNode.state` and `PhaseNode.state` are `Option<String>`, while `TournamentNode.state`, `PhaseGroupNode.state`, and `SetNode.state` are `Option<i32>`. Serde deserializes each naturally without additional adapters.

### Pagination: use `totalPages`, not `total`

`pageInfo.totalPages` is reliably present. `pageInfo.total` (the object count) is sometimes `null` depending on the endpoint. Always stop pagination when `page > totalPages`.

### DQ detection via score value

A set slot with `standing.stats.score.value == -1` is a disqualification, not a real score. Filter out any set where either slot has a score of `-1` before computing upset factor. This is distinct from `Entrant.isDisqualified` (which marks the entrant's overall DQ status, not per-set).

### `hasPlaceholder` sets

Sets with `hasPlaceholder: true` do not have real entrants in both slots yet (e.g. byes in early rounds). Skip these during import.
