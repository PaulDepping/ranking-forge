import type { PageServerLoad } from "./$types";
import type { HeadToHeadEntry, RankingPlayer } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const [h2hRes, playersRes] = await Promise.all([
    api.get(`/projects/${params.id}/rankings/${params.rid}/head-to-head`),
    api.get(`/projects/${params.id}/rankings/${params.rid}/players`),
  ]);
  const h2h: HeadToHeadEntry[] = h2hRes.ok ? await h2hRes.json() : [];
  const rawPlayers: RankingPlayer[] = playersRes.ok
    ? await playersRes.json()
    : [];
  // Normalize to { id, name } for the h2h page
  const players = rawPlayers.map((p) => ({ id: p.player_id, name: p.name }));
  return { h2h, players, wide: true };
};
