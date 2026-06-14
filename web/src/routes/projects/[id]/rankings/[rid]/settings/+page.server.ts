import { error, fail, redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ parent }) => {
  const { project } = await parent();
  const role = project.user_role;
  if (role !== "editor" && role !== "owner") {
    error(403, { message: "forbidden" });
  }
  return {};
};

export const actions = {
  save: async ({ request, params, locals }) => {
    const data = await request.formData();
    const name = ((data.get("name") as string) ?? "").trim();
    const description =
      ((data.get("description") as string) || "").trim() || undefined;
    const publishedRaw = data.get("published");
    const published =
      publishedRaw !== null ? publishedRaw === "true" : undefined;

    if (!name) return fail(422, { saveError: "Name is required" });

    const res = await locals.api.patch(
      `/projects/${params.id}/rankings/${params.rid}`,
      { name, description, published },
    );
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      return fail(res.status, {
        saveError: (body as { message?: string }).message ?? "Save failed",
      });
    }
    return { saved: true };
  },

  saveAlgorithmConfig: async ({ request, params, locals }) => {
    const data = await request.formData();
    const algorithm = data.get("algorithm") as string;

    let algorithm_config: Record<string, unknown>;
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
    } else {
      return fail(422, { algoError: "Invalid algorithm" });
    }

    const patchRes = await locals.api.patch(
      `/projects/${params.id}/rankings/${params.rid}`,
      { algorithm_config },
    );
    if (!patchRes.ok) {
      const body = await patchRes.json().catch(() => ({}));
      return fail(patchRes.status, {
        algoError:
          (body as { message?: string }).message ?? "Failed to save config",
      });
    }

    const recomputeRes = await locals.api.post(
      `/projects/${params.id}/rankings/${params.rid}/recompute`,
    );
    if (!recomputeRes.ok) {
      return fail(recomputeRes.status, {
        algoError: "Config saved but failed to enqueue recompute",
      });
    }

    return { algoSaved: true };
  },

  delete: async ({ params, locals }) => {
    const res = await locals.api.delete(
      `/projects/${params.id}/rankings/${params.rid}`,
    );
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      return fail(res.status, {
        deleteError: (body as { message?: string }).message ?? "Delete failed",
      });
    }
    redirect(303, `/projects/${params.id}`);
  },
} satisfies Actions;
