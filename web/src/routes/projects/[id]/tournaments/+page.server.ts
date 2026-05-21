import type { PageServerLoad } from "./$types";
import type { Tournament } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}/tournaments`);
  const tournaments: Tournament[] = res.ok ? await res.json() : [];
  return { tournaments };
};
