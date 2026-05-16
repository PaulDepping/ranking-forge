import { describe, it, expect } from 'vitest';
import type { Tournament, TournamentEvent } from '$lib/types';

type BracketTypeState = 'neutral' | 'required' | 'excluded';

function makeEvent(overrides: Partial<TournamentEvent> = {}): TournamentEvent {
    return {
        id: 'e1', startgg_id: 1, name: 'Melee Singles',
        game_name: null, num_entrants: 100, start_at: null,
        included: true, event_type: 1, bracket_types: ['DOUBLE_ELIMINATION'],
        ...overrides,
    };
}

function makeTournament(events: TournamentEvent[], overrides: Partial<Tournament> = {}): Tournament {
    return {
        id: 't1', startgg_id: 1, name: 'Genesis 10', slug: 'tournament/genesis-10',
        city: 'San Jose', addr_state: 'CA', country_code: 'US',
        venue_name: null, online: false,
        start_at: '2025-01-12T00:00:00Z', end_at: null,
        events,
        ...overrides,
    };
}

// Standalone filter functions (same logic as in +page.svelte, with explicit params)
function tournamentVisible(
    t: Tournament,
    venueFilter: 'all' | 'online' | 'offline',
    dateFrom: string,
    dateTo: string,
): boolean {
    if (venueFilter === 'online' && !t.online) return false;
    if (venueFilter === 'offline' && t.online) return false;
    if (dateFrom && t.start_at && t.start_at.slice(0, 10) < dateFrom) return false;
    if (dateTo && t.start_at && t.start_at.slice(0, 10) > dateTo) return false;
    return true;
}

function eventVisible(
    e: TournamentEvent,
    t: Tournament,
    search: string,
    minEntrants: number | null,
    maxEntrants: number | null,
    eventType: 'all' | 'singles' | 'teams',
    bracketFilter: Record<string, BracketTypeState>,
): boolean {
    if (search.trim()) {
        const q = search.trim().toLowerCase();
        if (!e.name.toLowerCase().includes(q) && !t.name.toLowerCase().includes(q)) return false;
    }
    if (+minEntrants > 0 && (e.num_entrants ?? Infinity) < +minEntrants) return false;
    if (+maxEntrants > 0 && (e.num_entrants ?? 0) > +maxEntrants) return false;
    if (eventType === 'singles' && e.event_type !== null && e.event_type !== 1) return false;
    if (eventType === 'teams' && e.event_type !== null && e.event_type !== 2) return false;

    const required = Object.entries(bracketFilter)
        .filter(([, s]) => s === 'required')
        .map(([t]) => t);
    const excluded = Object.entries(bracketFilter)
        .filter(([, s]) => s === 'excluded')
        .map(([t]) => t);

    if (required.length > 0 || excluded.length > 0) {
        if (e.bracket_types.length === 0) return true;
        for (const r of required) {
            if (!e.bracket_types.includes(r)) return false;
        }
        for (const x of excluded) {
            if (e.bracket_types.includes(x)) return false;
        }
    }

    return true;
}

const RARE_TYPES = [
    'EXHIBITION', 'RACE', 'CIRCUIT', 'CUSTOM_SCHEDULE', 'ELIMINATION_ROUNDS',
] as const;

function rareActiveCount(bracketFilter: Record<string, BracketTypeState>): number {
    return RARE_TYPES.filter(t => bracketFilter[t] === 'required' || bracketFilter[t] === 'excluded').length;
}

describe('tournament filter', () => {
    it('venue filter hides online tournaments', () => {
        const t = makeTournament([], { online: true });
        expect(tournamentVisible(t, 'offline', '', '')).toBe(false);
        expect(tournamentVisible(t, 'online', '', '')).toBe(true);
    });

    it('date range filter hides tournaments outside range', () => {
        const t = makeTournament([], { start_at: '2024-06-01T00:00:00Z' });
        expect(tournamentVisible(t, 'all', '2025-01-01', '')).toBe(false);
        expect(tournamentVisible(t, 'all', '2024-01-01', '2024-12-31')).toBe(true);
        // boundary: start_at on exactly dateTo day must pass
        expect(tournamentVisible(t, 'all', '', '2024-06-01')).toBe(true);
    });

    it('null start_at passes date filter', () => {
        const t = makeTournament([], { start_at: null });
        expect(tournamentVisible(t, 'all', '2025-01-01', '2025-12-31')).toBe(true);
    });

    it('name search matches event name', () => {
        const t = makeTournament([]);
        const e = makeEvent({ name: 'Melee Doubles' });
        expect(eventVisible(e, t, 'doubles', null, null, 'all', {})).toBe(true);
        expect(eventVisible(e, t, 'singles', null, null, 'all', {})).toBe(false);
    });

    it('name search on tournament name shows all events', () => {
        const t = makeTournament([]);
        const e = makeEvent({ name: 'Melee Doubles' });
        expect(eventVisible(e, t, 'genesis', null, null, 'all', {})).toBe(true);
    });

    it('entrant range filter', () => {
        const t = makeTournament([]);
        const small = makeEvent({ num_entrants: 16 });
        const large = makeEvent({ num_entrants: 512 });
        expect(eventVisible(small, t, '', 32, null, 'all', {})).toBe(false);
        expect(eventVisible(large, t, '', 32, 200, 'all', {})).toBe(false);
        expect(eventVisible(large, t, '', 32, null, 'all', {})).toBe(true);
    });

    it('null num_entrants passes min/max filter', () => {
        const t = makeTournament([]);
        const e = makeEvent({ num_entrants: null });
        expect(eventVisible(e, t, '', 32, 100, 'all', {})).toBe(true);
    });

    it('eventType singles filter', () => {
        const t = makeTournament([]);
        const singles = makeEvent({ event_type: 1 });
        const teams = makeEvent({ event_type: 2 });
        expect(eventVisible(singles, t, '', null, null, 'singles', {})).toBe(true);
        expect(eventVisible(teams, t, '', null, null, 'singles', {})).toBe(false);
    });

    it('null event_type passes all eventType filters', () => {
        const t = makeTournament([]);
        const e = makeEvent({ event_type: null });
        expect(eventVisible(e, t, '', null, null, 'singles', {})).toBe(true);
        expect(eventVisible(e, t, '', null, null, 'teams', {})).toBe(true);
    });
});

describe('bracket type filter', () => {
    it('all neutral — no filtering applied', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: ['MATCHMAKING'] });
        expect(eventVisible(e, t, '', null, null, 'all', {})).toBe(true);
    });

    it('required type present — passes', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: ['DOUBLE_ELIMINATION'] });
        expect(eventVisible(e, t, '', null, null, 'all', { DOUBLE_ELIMINATION: 'required' })).toBe(true);
    });

    it('required type absent — filtered out', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: ['ROUND_ROBIN'] });
        expect(eventVisible(e, t, '', null, null, 'all', { DOUBLE_ELIMINATION: 'required' })).toBe(false);
    });

    it('excluded type present — filtered out', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: ['MATCHMAKING'] });
        expect(eventVisible(e, t, '', null, null, 'all', { MATCHMAKING: 'excluded' })).toBe(false);
    });

    it('excluded type absent — passes', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: ['DOUBLE_ELIMINATION'] });
        expect(eventVisible(e, t, '', null, null, 'all', { MATCHMAKING: 'excluded' })).toBe(true);
    });

    it('multiple required types — event must have all of them', () => {
        const t = makeTournament([]);
        const hasAll = makeEvent({ bracket_types: ['ROUND_ROBIN', 'DOUBLE_ELIMINATION'] });
        const missingOne = makeEvent({ bracket_types: ['DOUBLE_ELIMINATION'] });
        const filter: Record<string, BracketTypeState> = {
            ROUND_ROBIN: 'required',
            DOUBLE_ELIMINATION: 'required',
        };
        expect(eventVisible(hasAll, t, '', null, null, 'all', filter)).toBe(true);
        expect(eventVisible(missingOne, t, '', null, null, 'all', filter)).toBe(false);
    });

    it('required + excluded on different types — excluded wins when both present', () => {
        const t = makeTournament([]);
        // Event has the required type but also the excluded type → rejected
        const e = makeEvent({ bracket_types: ['DOUBLE_ELIMINATION', 'MATCHMAKING'] });
        const filter: Record<string, BracketTypeState> = {
            DOUBLE_ELIMINATION: 'required',
            MATCHMAKING: 'excluded',
        };
        expect(eventVisible(e, t, '', null, null, 'all', filter)).toBe(false);
    });

    it('empty bracket_types passes regardless of filter state', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: [] });
        const filter: Record<string, BracketTypeState> = {
            DOUBLE_ELIMINATION: 'required',
            MATCHMAKING: 'excluded',
        };
        expect(eventVisible(e, t, '', null, null, 'all', filter)).toBe(true);
    });
});

describe('rareActiveCount', () => {
    it('returns 0 when all types are neutral', () => {
        const filter: Record<string, BracketTypeState> = {};
        expect(rareActiveCount(filter)).toBe(0);
    });

    it('counts required rare types', () => {
        const filter: Record<string, BracketTypeState> = {
            EXHIBITION: 'required',
            RACE: 'excluded',
        };
        expect(rareActiveCount(filter)).toBe(2);
    });

    it('ignores common types', () => {
        const filter: Record<string, BracketTypeState> = {
            DOUBLE_ELIMINATION: 'required',
        };
        expect(rareActiveCount(filter)).toBe(0);
    });
});
