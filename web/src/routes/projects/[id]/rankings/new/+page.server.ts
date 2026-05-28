import { redirect, fail } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ parent }) => {
  const { project } = await parent();
  if (project.user_role !== "owner" && project.user_role !== "editor") {
    redirect(303, `/projects/${project.id}`);
  }
  return {};
};

export const actions = {
  default: async ({ request, params, locals }) => {
    const { api } = locals;
    const data = await request.formData();
    const name = (data.get("name") as string)?.trim();
    const description =
      ((data.get("description") as string) || "").trim() || undefined;

    if (!name) return fail(422, { error: "Name is required" });

    const res = await api.post(`/projects/${params.id}/rankings`, {
      name,
      description,
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      return fail(res.status, {
        error:
          (body as { message?: string }).message ?? "Failed to create ranking",
      });
    }
    const ranking = await res.json();
    redirect(303, `/projects/${params.id}/rankings/${ranking.id}/players`);
  },
} satisfies Actions;
