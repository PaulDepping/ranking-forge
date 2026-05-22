import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import TournamentTab from "./TournamentTab.svelte";
import type { TournamentData } from "$lib/types";

describe("TournamentTab", () => {
  const originalFetch = global.fetch;

  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    global.fetch = originalFetch;
  });

  it("renders without error", () => {
    const { component } = render(TournamentTab, {
      props: {
        projectId: "proj-1",
        players: [],
        onClose: () => {},
      },
    });

    expect(component).toBeDefined();
  });

  it("shows 'brackets haven't been published yet' for CREATED events with no entrants", async () => {
    const tournamentData: TournamentData = {
      all_participants: [],
      events: [
        {
          id: 1,
          name: "Melee Singles",
          state: "CREATED",
          entrants: [],
        },
      ],
    };

    const mockFetch = vi.fn().mockImplementation(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve(tournamentData),
      } as Response),
    );
    global.fetch = mockFetch;
    // @ts-ignore
    globalThis.fetch = mockFetch;

    const { container } = render(TournamentTab, {
      props: {
        projectId: "proj-1",
        players: [],
        onClose: () => {},
      },
    });

    // Enter tournament slug
    const input = container.querySelector(
      'input[placeholder*="genesis"]',
    ) as HTMLInputElement;
    input.value = "test-tournament";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));

    // Wait a tick for Svelte to update
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Click Fetch button
    const fetchButton = screen.getByRole("button", { name: "Fetch" });
    await fireEvent.click(fetchButton);

    // Wait for tabs to appear
    await waitFor(
      () => {
        expect(
          screen.queryByRole("tab", { name: "Melee Singles" }),
        ).toBeInTheDocument();
      },
      { timeout: 3000 },
    );

    // Click the Melee Singles event tab
    const meleeTrigger = screen.getByRole("tab", { name: "Melee Singles" });
    await fireEvent.click(meleeTrigger);

    // Should show the CREATED state message
    await waitFor(
      () => {
        expect(
          screen.getByText("This event's brackets haven't been published yet"),
        ).toBeInTheDocument();
      },
      { timeout: 3000 },
    );
  });

  it("shows 'No entrants found for this event' for non-CREATED events with no entrants", async () => {
    const tournamentData: TournamentData = {
      all_participants: [],
      events: [
        {
          id: 2,
          name: "Melee Doubles",
          state: "COMPLETED",
          entrants: [],
        },
      ],
    };

    const mockFetch = vi.fn().mockImplementation(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve(tournamentData),
      } as Response),
    );
    global.fetch = mockFetch;
    // @ts-ignore
    globalThis.fetch = mockFetch;

    const { container } = render(TournamentTab, {
      props: {
        projectId: "proj-1",
        players: [],
        onClose: () => {},
      },
    });

    // Enter tournament slug
    const input = container.querySelector(
      'input[placeholder*="genesis"]',
    ) as HTMLInputElement;
    input.value = "test-tournament";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));

    // Wait a tick for Svelte to update
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Click Fetch button
    const fetchButton = screen.getByRole("button", { name: "Fetch" });
    await fireEvent.click(fetchButton);

    // Wait for tabs to appear
    await waitFor(
      () => {
        expect(
          screen.queryByRole("tab", { name: "Melee Doubles" }),
        ).toBeInTheDocument();
      },
      { timeout: 3000 },
    );

    // Click the Melee Doubles event tab
    const doublesTrigger = screen.getByRole("tab", {
      name: "Melee Doubles",
    });
    await fireEvent.click(doublesTrigger);

    // Should show the non-CREATED state message
    await waitFor(
      () => {
        expect(
          screen.getByText("No entrants found for this event"),
        ).toBeInTheDocument();
      },
      { timeout: 3000 },
    );
  });

  it("shows entrant list when event tab has entrants", async () => {
    const tournamentData: TournamentData = {
      all_participants: [
        {
          startgg_user_id: 101,
          handle: "Player1",
          name: "Player One",
        },
      ],
      events: [
        {
          id: 3,
          name: "Melee Singles",
          state: "COMPLETED",
          entrants: [
            {
              startgg_user_id: 101,
              handle: "Player1",
              name: "Player One",
              seed: 1,
              placement: null,
            },
          ],
        },
      ],
    };

    const mockFetch = vi.fn().mockImplementation(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve(tournamentData),
      } as Response),
    );
    global.fetch = mockFetch;
    // @ts-ignore
    globalThis.fetch = mockFetch;

    const { container } = render(TournamentTab, {
      props: {
        projectId: "proj-1",
        players: [],
        onClose: () => {},
      },
    });

    // Enter tournament slug
    const input = container.querySelector(
      'input[placeholder*="genesis"]',
    ) as HTMLInputElement;
    input.value = "test-tournament";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));

    // Wait a tick for Svelte to update
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Click Fetch button
    const fetchButton = screen.getByRole("button", { name: "Fetch" });
    await fireEvent.click(fetchButton);

    // Wait for tabs to appear
    await waitFor(
      () => {
        expect(
          screen.queryByRole("tab", { name: "Melee Singles" }),
        ).toBeInTheDocument();
      },
      { timeout: 3000 },
    );

    // Click the Melee Singles event tab
    const meleeTrigger = screen.getByRole("tab", { name: "Melee Singles" });
    await fireEvent.click(meleeTrigger);

    // Should show the entrant list and not show empty state message
    await waitFor(
      () => {
        expect(screen.getByText("Player One")).toBeInTheDocument();
      },
      { timeout: 3000 },
    );

    expect(
      screen.queryByText("This event's brackets haven't been published yet"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("No entrants found for this event"),
    ).not.toBeInTheDocument();
  });

  it("shows entrant list on 'All' tab even with empty event", async () => {
    const tournamentData: TournamentData = {
      all_participants: [
        {
          startgg_user_id: 102,
          handle: "GlobalPlayer",
          name: "Global Player",
        },
      ],
      events: [
        {
          id: 4,
          name: "Empty Event",
          state: "CREATED",
          entrants: [],
        },
      ],
    };

    const mockFetch = vi.fn().mockImplementation(() =>
      Promise.resolve({
        ok: true,
        json: () => Promise.resolve(tournamentData),
      } as Response),
    );
    global.fetch = mockFetch;
    // @ts-ignore
    globalThis.fetch = mockFetch;

    const { container } = render(TournamentTab, {
      props: {
        projectId: "proj-1",
        players: [],
        onClose: () => {},
      },
    });

    // Enter tournament slug
    const input = container.querySelector(
      'input[placeholder*="genesis"]',
    ) as HTMLInputElement;
    input.value = "test-tournament";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.dispatchEvent(new Event("change", { bubbles: true }));

    // Wait a tick for Svelte to update
    await new Promise((resolve) => setTimeout(resolve, 0));

    // Click Fetch button
    const fetchButton = screen.getByRole("button", { name: "Fetch" });
    await fireEvent.click(fetchButton);

    // Wait for data to load
    await waitFor(
      () => {
        expect(screen.queryByRole("tab", { name: "All" })).toBeInTheDocument();
      },
      { timeout: 3000 },
    );

    // "All" tab should be selected by default
    // Verify the entrant list is visible
    await waitFor(
      () => {
        expect(screen.getByText("Global Player")).toBeInTheDocument();
      },
      { timeout: 3000 },
    );

    // Should NOT show empty state messages on "All" tab
    expect(
      screen.queryByText("This event's brackets haven't been published yet"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText("No entrants found for this event"),
    ).not.toBeInTheDocument();
  });
});
