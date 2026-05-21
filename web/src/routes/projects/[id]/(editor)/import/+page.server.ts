import { fail } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import type { Job } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}/import`);
  const job: Job | null = res.ok ? await res.json() : null;
  return { job };
};

export const actions: Actions = {
  default: async ({ params, locals, request }) => {
    const { api } = locals;
    const data = await request.formData();
    const afterDate = data.get("after_date") as string | null;
    const beforeDate = data.get("before_date") as string | null;
    const body: Record<string, string> = {};
    if (afterDate) body.after_date = afterDate;
    if (beforeDate) body.before_date = beforeDate;
    const res = await api.post(
      `/projects/${params.id}/import`,
      Object.keys(body).length ? body : undefined,
    );
    if (!res.ok) {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to start import" }));
      return fail(res.status, { error: err.message });
    }
    const job: Job = await res.json();
    return { job };
  },
};
