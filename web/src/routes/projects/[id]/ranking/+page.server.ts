import type { PageServerLoad } from "./$types";
import type { Player, PlayerStats } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const [playersRes, statsRes] = await Promise.all([
    api.get(`/projects/${params.id}/players`),
    api.get(`/projects/${params.id}/stats`),
  ]);
  const players: Player[] = playersRes.ok ? await playersRes.json() : [];
  const stats: PlayerStats[] = statsRes.ok ? await statsRes.json() : [];
  return { players, stats };
};
