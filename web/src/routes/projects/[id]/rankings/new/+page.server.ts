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
    const algorithm = (data.get("algorithm") as string) || null;

    if (!name) return fail(422, { error: "Name is required" });

    // Build algorithm_config from form fields when an algorithm is selected
    let algorithm_config: Record<string, unknown> | undefined;
    if (algorithm === "elo") {
      const k = parseFloat(data.get("elo_k") as string);
      const initial = parseFloat(data.get("elo_initial") as string);
      algorithm_config = {
        k_factor: isNaN(k) ? 32 : k,
        initial_rating: isNaN(initial) ? 1500 : initial,
      };
    } else if (algorithm === "glicko2") {
      const tau = parseFloat(data.get("g2_tau") as string);
      const rd = parseFloat(data.get("g2_rd") as string);
      const sigma = parseFloat(data.get("g2_sigma") as string);
      algorithm_config = {
        tau: isNaN(tau) ? 0.5 : tau,
        initial_rd: isNaN(rd) ? 350 : rd,
        initial_volatility: isNaN(sigma) ? 0.06 : sigma,
      };
    }

    const res = await api.post(`/projects/${params.id}/rankings`, {
      name,
      description,
      algorithm: algorithm || undefined,
      algorithm_config,
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
