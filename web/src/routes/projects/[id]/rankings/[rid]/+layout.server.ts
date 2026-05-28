import { error } from "@sveltejs/kit";
import type { LayoutServerLoad } from "./$types";
import type { Ranking } from "$lib/types";

export const load: LayoutServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}/rankings/${params.rid}`);
  if (!res.ok) {
    error(res.status === 404 ? 404 : res.status, {
      message: res.status === 404 ? "not_found" : "error",
    });
  }
  const ranking: Ranking = await res.json();
  return { ranking };
};
