import { redirect } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";
import type { Ranking } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals, parent }) => {
  const { project } = await parent();
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}/rankings`);
  const rankings: Ranking[] = res.ok ? await res.json() : [];

  if (rankings.length === 1) {
    const role = project.user_role;
    const rid = rankings[0].id;
    if (role === "editor" || role === "owner") {
      redirect(303, `/projects/${params.id}/rankings/${rid}/players`);
    }
    redirect(303, `/projects/${params.id}/rankings/${rid}/ranking`);
  }

  return { rankings };
};
