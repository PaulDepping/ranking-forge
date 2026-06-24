import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen } from "@testing-library/svelte";
import SetDetailModal from "./SetDetailModal.svelte";
import type { SetRecord } from "$lib/types";

const baseSet: SetRecord = {
  opponent_id: "p2",
  opponent_name: "Bob",
  upset_factor: 2,
  winner_score: 3,
  loser_score: 1,
  tournament_name: "Genesis 9",
  tournament_handle: "genesis-9",
  event_name: "Melee Singles",
  round_name: "Winners Finals",
  completed_at: "2024-01-20T18:00:00Z",
  is_dq: false,
  vod_url: null,
  startgg_set_id: 12345,
  winner_seed: 1,
  loser_seed: 12,
  phase_name: null,
  pool_identifier: null,
  winner_placement: null,
  loser_placement: null,
  location: null,
  num_entrants: null,
  event_handle: "melee-singles",
};

// bits-ui's body-scroll-lock schedules a setTimeout during Dialog cleanup.
// Without fake timers that timeout fires after jsdom tears down, causing
// "document is not defined". Fake timers drain the queue while DOM is alive.
beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.runAllTimers();
  vi.useRealTimers();
});

describe("SetDetailModal", () => {
  it("renders nothing when set is null", () => {
    render(SetDetailModal, {
      props: {
        set: null,
        isWin: false,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.queryByText("Genesis 9")).not.toBeInTheDocument();
  });

  it("shows player names in title", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Alice vs Bob")).toBeInTheDocument();
  });

  it("shows Win in subtitle without score", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Win")).toBeInTheDocument();
  });

  it("shows Loss in subtitle without score", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: false,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Loss")).toBeInTheDocument();
  });

  it("shows score cells labelled with player names", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Alice score")).toBeInTheDocument();
    expect(screen.getByText("Bob score")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
  });

  it("shows score cells swapped for loss perspective", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: false,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Alice score")).toBeInTheDocument();
    expect(screen.getByText("Bob score")).toBeInTheDocument();
  });

  it("hides score row when both scores are null", () => {
    render(SetDetailModal, {
      props: {
        set: { ...baseSet, winner_score: null, loser_score: null },
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.queryByText("Alice score")).not.toBeInTheDocument();
  });

  it("shows seed cells labelled with player names", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Alice seed")).toBeInTheDocument();
    expect(screen.getByText("Bob seed")).toBeInTheDocument();
    expect(screen.getByText("#1")).toBeInTheDocument();
    expect(screen.getByText("#12")).toBeInTheDocument();
  });

  it("hides seed row when both seeds are null", () => {
    render(SetDetailModal, {
      props: {
        set: { ...baseSet, winner_seed: null, loser_seed: null },
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.queryByText("Alice seed")).not.toBeInTheDocument();
  });

  it("shows tournament and event name combined", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Genesis 9 · Melee Singles")).toBeInTheDocument();
  });

  it("shows round in tournament section", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Winners Finals")).toBeInTheDocument();
  });

  it("shows upset factor", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("2")).toBeInTheDocument();
  });

  it("shows phase when present", () => {
    render(SetDetailModal, {
      props: {
        set: { ...baseSet, phase_name: "Top 8" },
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Top 8")).toBeInTheDocument();
  });

  it("appends pool identifier to phase label", () => {
    render(SetDetailModal, {
      props: {
        set: { ...baseSet, phase_name: "Pools", pool_identifier: "Pool A" },
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Pools · Pool A")).toBeInTheDocument();
  });

  it("hides phase row when phase_name is null", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.queryByText("Phase")).not.toBeInTheDocument();
  });

  it("shows location when present", () => {
    render(SetDetailModal, {
      props: {
        set: { ...baseSet, location: "Austin, TX" },
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("Austin, TX")).toBeInTheDocument();
  });

  it("shows num_entrants when present", () => {
    render(SetDetailModal, {
      props: {
        set: { ...baseSet, num_entrants: 256 },
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("256")).toBeInTheDocument();
  });

  it("shows final placements section with player names and ordinals", () => {
    render(SetDetailModal, {
      props: {
        set: { ...baseSet, winner_placement: 1, loser_placement: 3 },
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.getByText("1st")).toBeInTheDocument();
    expect(screen.getByText("3rd")).toBeInTheDocument();
  });

  it("hides placements section when both placements are null", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(screen.queryByText("Final Placements")).not.toBeInTheDocument();
  });

  it("set link points to /set/{startgg_set_id}", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    const link = screen.getByRole("link", { name: /View set on start\.gg/ });
    expect(link).toHaveAttribute(
      "href",
      "https://www.start.gg/tournament/genesis-9/event/melee-singles/set/12345",
    );
  });

  it("hides VOD link when vod_url is null", () => {
    render(SetDetailModal, {
      props: {
        set: baseSet,
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    expect(
      screen.queryByRole("link", { name: /Watch VOD/ }),
    ).not.toBeInTheDocument();
  });

  it("shows VOD link when vod_url is present", () => {
    render(SetDetailModal, {
      props: {
        set: { ...baseSet, vod_url: "https://youtube.com/watch?v=abc" },
        isWin: true,
        currentPlayerName: "Alice",
        onClose: () => {},
      },
    });
    const link = screen.getByRole("link", { name: /Watch VOD/ });
    expect(link).toHaveAttribute("href", "https://youtube.com/watch?v=abc");
  });
});
