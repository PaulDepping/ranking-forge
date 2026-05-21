import type { PageServerLoad } from "./$types";
import type { Project } from "$lib/types";

export const load: PageServerLoad = async ({ locals }) => {
  const { api } = locals;
  const res = await api.get("/projects");
  if (!res.ok) return { projects: [] as Project[] };
  const projects: Project[] = await res.json();
  return { projects };
};
