import { error } from "@sveltejs/kit";
import type { LayoutServerLoad } from "./$types";
import { makeApi } from "$lib/api";
import type { Project } from "$lib/types";
import { env } from "$env/dynamic/private";

export const load: LayoutServerLoad = async ({
  fetch,
  params,
  cookies,
  locals,
}) => {
  const api = makeApi(fetch, env.INTERNAL_API_URL, cookies.get("session_id"));
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
