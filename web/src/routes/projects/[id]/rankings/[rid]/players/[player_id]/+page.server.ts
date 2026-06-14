import { error } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";
import type {
  PlayerStats,
  RankingPlayer,
  TournamentAttendance,
} from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;

  const [statsRes, tournamentsRes, rankingPlayersRes] = await Promise.all([
    api.get(
      `/projects/${params.id}/rankings/${params.rid}/stats/${params.player_id}`,
    ),
    api.get(
      `/projects/${params.id}/rankings/${params.rid}/players/${params.player_id}/tournaments`,
    ),
    api.get(`/projects/${params.id}/rankings/${params.rid}/players`),
  ]);

  if (!statsRes.ok) {
    if (statsRes.status === 404) {
      error(404, { message: "not_found" });
    }
    error(statsRes.status, { message: "error" });
  }

  const stats: PlayerStats = await statsRes.json();

  if (!tournamentsRes.ok) {
    error(tournamentsRes.status, "Failed to load tournament history");
  }

  const tournaments: TournamentAttendance[] = await tournamentsRes.json();

  const rankingPlayers: RankingPlayer[] = rankingPlayersRes.ok
    ? await rankingPlayersRes.json()
    : [];
  const trackedPlayerIds = new Set(rankingPlayers.map((p) => p.player_id));

  return {
    stats,
    tournaments,
    trackedPlayerIds,
    projectId: params.id,
    rankingId: params.rid,
  };
};
