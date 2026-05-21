import { error } from "@sveltejs/kit";
import type { LayoutServerLoad } from "./$types";
import type { Project } from "$lib/types";

export const load: LayoutServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}`);
  if (!res.ok) {
    if (res.status === 404) {
      if (!locals.user) {
        error(404, { message: "private_project" });
      }
      error(404, { message: "not_found" });
    }
    error(res.status, { message: "error" });
  }
  const project: Project = await res.json();
  return { project };
};
