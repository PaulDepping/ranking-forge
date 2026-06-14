import type { PageServerLoad } from "./$types";
import type { RankingPlayerWithScore, PlayerStats } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const [playersRes, statsRes] = await Promise.all([
    api.get(`/projects/${params.id}/rankings/${params.rid}/ranking`),
    api.get(`/projects/${params.id}/rankings/${params.rid}/stats`),
  ]);
  const players: RankingPlayerWithScore[] = playersRes.ok
    ? await playersRes.json()
    : [];
  const stats: PlayerStats[] = statsRes.ok ? await statsRes.json() : [];
  return { players, stats };
};
