import { redirect } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ params, parent }) => {
  const { project } = await parent();
  const role = project.user_role;
  if (role === "editor" || role === "owner") {
    redirect(303, `/projects/${params.id}/rankings/${params.rid}/players`);
  }
  redirect(303, `/projects/${params.id}/rankings/${params.rid}/ranking`);
};
