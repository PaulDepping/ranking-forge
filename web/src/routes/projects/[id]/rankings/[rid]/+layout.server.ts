import { error } from "@sveltejs/kit";
import type { LayoutServerLoad } from "./$types";
import type { Ranking } from "$lib/types";

export const load: LayoutServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const [rankingRes, rankingsRes] = await Promise.all([
    api.get(`/projects/${params.id}/rankings/${params.rid}`),
    api.get(`/projects/${params.id}/rankings`),
  ]);
  if (!rankingRes.ok) {
    error(rankingRes.status === 404 ? 404 : rankingRes.status, {
      message: rankingRes.status === 404 ? "not_found" : "error",
    });
  }
  const ranking: Ranking = await rankingRes.json();
  const rankings: Ranking[] = rankingsRes.ok ? await rankingsRes.json() : [];
  return { ranking, rankings };
};
