import { redirect } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";
import type { Player, RankingPlayer } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals, parent }) => {
  const { project } = await parent();
  if (project.user_role !== "owner" && project.user_role !== "editor") {
    redirect(303, `/projects/${params.id}/rankings/${params.rid}/ranking`);
  }

  const { api } = locals;
  const [poolRes, rankingPlayersRes] = await Promise.all([
    api.get(`/projects/${params.id}/players`),
    api.get(`/projects/${params.id}/rankings/${params.rid}/players`),
  ]);

  const pool: Player[] = poolRes.ok ? await poolRes.json() : [];
  const rankingPlayers: RankingPlayer[] = rankingPlayersRes.ok
    ? await rankingPlayersRes.json()
    : [];

  return { pool, rankingPlayers };
};
