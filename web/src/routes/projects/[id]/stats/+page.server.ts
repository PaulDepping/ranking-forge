import type { PageServerLoad } from "./$types";
import type { PlayerStats } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}/stats`);
  const stats: PlayerStats[] = res.ok ? await res.json() : [];
  return { stats, wide: true };
};
