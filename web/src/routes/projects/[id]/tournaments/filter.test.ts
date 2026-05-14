import { describe, it, expect } from 'vitest';
import type { Tournament, TournamentEvent } from '$lib/types';

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
    excludeLadder: boolean,
): boolean {
    if (search.trim()) {
        const q = search.trim().toLowerCase();
        if (!e.name.toLowerCase().includes(q) && !t.name.toLowerCase().includes(q)) return false;
    }
    if (+minEntrants > 0 && (e.num_entrants ?? Infinity) < +minEntrants) return false;
    if (+maxEntrants > 0 && (e.num_entrants ?? 0) > +maxEntrants) return false;
    if (eventType === 'singles' && e.event_type !== null && e.event_type !== 1) return false;
    if (eventType === 'teams' && e.event_type !== null && e.event_type !== 2) return false;
    if (excludeLadder && e.bracket_types.length > 0 &&
        e.bracket_types.every(bt => bt === 'MATCHMAKING')) return false;
    return true;
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
        expect(eventVisible(e, t, 'doubles', null, null, 'all', false)).toBe(true);
        expect(eventVisible(e, t, 'singles', null, null, 'all', false)).toBe(false);
    });

    it('name search on tournament name shows all events', () => {
        const t = makeTournament([]);
        const e = makeEvent({ name: 'Melee Doubles' });
        expect(eventVisible(e, t, 'genesis', null, null, 'all', false)).toBe(true);
    });

    it('entrant range filter', () => {
        const t = makeTournament([]);
        const small = makeEvent({ num_entrants: 16 });
        const large = makeEvent({ num_entrants: 512 });
        expect(eventVisible(small, t, '', 32, null, 'all', false)).toBe(false);
        expect(eventVisible(large, t, '', 32, 200, 'all', false)).toBe(false);
        expect(eventVisible(large, t, '', 32, null, 'all', false)).toBe(true);
    });

    it('null num_entrants passes min/max filter', () => {
        const t = makeTournament([]);
        const e = makeEvent({ num_entrants: null });
        expect(eventVisible(e, t, '', 32, 100, 'all', false)).toBe(true);
    });

    it('eventType singles filter', () => {
        const t = makeTournament([]);
        const singles = makeEvent({ event_type: 1 });
        const teams = makeEvent({ event_type: 2 });
        expect(eventVisible(singles, t, '', null, null, 'singles', false)).toBe(true);
        expect(eventVisible(teams, t, '', null, null, 'singles', false)).toBe(false);
    });

    it('null event_type passes all eventType filters', () => {
        const t = makeTournament([]);
        const e = makeEvent({ event_type: null });
        expect(eventVisible(e, t, '', null, null, 'singles', false)).toBe(true);
        expect(eventVisible(e, t, '', null, null, 'teams', false)).toBe(true);
    });

    it('excludeLadder only hides pure MATCHMAKING events', () => {
        const t = makeTournament([]);
        const ladder = makeEvent({ bracket_types: ['MATCHMAKING'] });
        const mixed = makeEvent({ bracket_types: ['ROUND_ROBIN', 'DOUBLE_ELIMINATION'] });
        const pools_bracket = makeEvent({ bracket_types: ['ROUND_ROBIN', 'MATCHMAKING'] });
        expect(eventVisible(ladder, t, '', null, null, 'all', true)).toBe(false);
        expect(eventVisible(mixed, t, '', null, null, 'all', true)).toBe(true);
        // pools_bracket has ROUND_ROBIN + MATCHMAKING — not ALL are MATCHMAKING, so it passes
        expect(eventVisible(pools_bracket, t, '', null, null, 'all', true)).toBe(true);
    });

    it('empty bracket_types passes excludeLadder filter', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: [] });
        expect(eventVisible(e, t, '', null, null, 'all', true)).toBe(true);
    });
});
