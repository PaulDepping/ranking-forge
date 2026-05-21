import type { PageServerLoad } from "./$types";
import type { HeadToHeadEntry, Player } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const [h2hRes, playersRes] = await Promise.all([
    api.get(`/projects/${params.id}/head-to-head`),
    api.get(`/projects/${params.id}/players`),
  ]);
  const h2h: HeadToHeadEntry[] = h2hRes.ok ? await h2hRes.json() : [];
  const players: Player[] = playersRes.ok ? await playersRes.json() : [];
  return { h2h, players };
};
