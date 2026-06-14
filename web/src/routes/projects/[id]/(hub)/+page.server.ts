import type { PageServerLoad } from "./$types";
import type { Ranking } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}/rankings`);
  const rankings: Ranking[] = res.ok ? await res.json() : [];
  return { rankings };
};
