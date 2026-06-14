import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, it, expect } from "vitest";
import Page from "./H2HTestWrapper.svelte";

import type { Project, Ranking } from "$lib/types";

const user = {
  id: "u1",
  email: "testuser@test.com",
  display_name: "testuser",
  has_startgg_key: false,
  created_at: "2026-01-01T00:00:00Z",
};

const ranking: Ranking = {
  id: "rank-1",
  project_id: "proj-1",
  name: "Main Ranking",
  description: null,
  published: false,
  created_at: "2026-01-01T00:00:00Z",
  user_role: "owner",
  algorithm: null,
  algorithm_config: {},
  include_external_results: false,
  result_sort: "upset_factor",
};
const rankings = [ranking];
const project: Project = {
  id: "proj-1",
  name: "Test Project",
  game_id: null,
  game_name: null,
  created_at: "2026-01-01T00:00:00Z",
  user_role: "owner",
  owner_has_startgg_key: true,
};

const players = [
  { id: "p1", name: "Alice" },
  { id: "p2", name: "Bob" },
  { id: "p3", name: "Charlie" },
];

const h2h = [
  { player_id: "p1", opponent_id: "p2", wins: 3, losses: 1 },
  { player_id: "p2", opponent_id: "p1", wins: 1, losses: 3 },
  { player_id: "p1", opponent_id: "p3", wins: 2, losses: 0 },
  { player_id: "p3", opponent_id: "p1", wins: 0, losses: 2 },
  { player_id: "p2", opponent_id: "p3", wins: 2, losses: 1 },
  { player_id: "p3", opponent_id: "p2", wins: 1, losses: 2 },
];

describe("H2H page", () => {
  it("renders player names in header row", () => {
    render(Page, { data: { user, project, ranking, rankings, players, h2h, wide: false } });
    expect(screen.getAllByText("Alice").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Bob").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Charlie").length).toBeGreaterThan(0);
  });

  it("renders win–loss records between players", () => {
    render(Page, { data: { user, project, ranking, rankings, players, h2h, wide: false } });
    // Alice vs Bob: 3–1 (and Bob vs Alice: 1–3)
    expect(screen.getByText("3–1")).toBeInTheDocument();
    expect(screen.getByText("1–3")).toBeInTheDocument();
    // Alice vs Charlie: 2–0
    expect(screen.getByText("2–0")).toBeInTheDocument();
    expect(screen.getByText("0–2")).toBeInTheDocument();
  });

  it("shows empty message when h2h data is absent", () => {
    render(Page, {
      data: {
        user,
        project,
        ranking,
        rankings,
        players: players.slice(0, 1),
        h2h: [],
        wide: false,
      },
    });
    expect(screen.getByText("No head-to-head data yet")).toBeInTheDocument();
    expect(
      screen.getByText("Import tournaments to generate head-to-head records."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("shows table footer note when data is present", () => {
    render(Page, { data: { user, project, ranking, rankings, players, h2h, wide: false } });
    expect(
      screen.getByText("Row player's record vs. column player"),
    ).toBeInTheDocument();
  });

  it("renders a dash for same-player diagonal cells", () => {
    render(Page, { data: { user, project, ranking, rankings, players, h2h, wide: false } });
    const dashCells = screen.getAllByText("—");
    // One dash per player (3 players → 3 diagonal cells)
    expect(dashCells.length).toBe(players.length);
  });

  it("renders non-diagonal cells as clickable buttons", () => {
    render(Page, { data: { user, project, ranking, rankings, players, h2h, wide: false } });
    // Alice vs Bob cell shows "3–1" as a button
    expect(screen.getByRole("button", { name: "3–1" })).toBeInTheDocument();
  });

  it("does not show popover content before any cell is clicked", () => {
    render(Page, { data: { user, project, ranking, rankings, players, h2h, wide: false } });
    expect(screen.queryByText(/wins ·/i)).not.toBeInTheDocument();
  });
});
