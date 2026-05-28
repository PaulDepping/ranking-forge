import type { PageServerLoad } from "./$types";
import type { RankingPlayer, PlayerStats } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const [playersRes, statsRes] = await Promise.all([
    api.get(`/projects/${params.id}/rankings/${params.rid}/players`),
    api.get(`/projects/${params.id}/rankings/${params.rid}/stats`),
  ]);
  const players: RankingPlayer[] = playersRes.ok ? await playersRes.json() : [];
  const stats: PlayerStats[] = statsRes.ok ? await statsRes.json() : [];
  return { players, stats };
};
