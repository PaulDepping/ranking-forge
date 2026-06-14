import { fail } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import type { Player } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}/players`);
  const players: Player[] = res.ok ? await res.json() : [];
  return { players };
};

export const actions: Actions = {
  addPlayer: async ({ request, params, locals }) => {
    const data = await request.formData();
    const name = (data.get("name") as string)?.trim();
    if (!name) return fail(422, { addError: "Player name is required" });

    const { api } = locals;
    const res = await api.post(`/projects/${params.id}/players`, { name });
    if (!res.ok) {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to add player" }));
      return fail(res.status, { addError: err.message });
    }
  },

  deletePlayer: async ({ request, params, locals }) => {
    const data = await request.formData();
    const pid = data.get("pid") as string;
    const { api } = locals;
    const res = await api.delete(`/projects/${params.id}/players/${pid}`);
    if (!res.ok)
      return fail(res.status, { deleteError: "Failed to delete player" });
  },

  renamePlayer: async ({ request, params, locals }) => {
    const data = await request.formData();
    const pid = data.get("pid") as string;
    const name = (data.get("name") as string)?.trim();
    if (!name)
      return fail(422, { renameError: "Name is required", renamePid: pid });

    const { api } = locals;
    const res = await api.patch(`/projects/${params.id}/players/${pid}`, {
      name,
    });
    if (!res.ok) {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to rename player" }));
      return fail(res.status, { renameError: err.message, renamePid: pid });
    }
  },

  linkAccount: async ({ request, params, locals }) => {
    const data = await request.formData();
    const pid = data.get("pid") as string;
    const handle = (data.get("handle") as string)?.trim();
    if (!handle)
      return fail(422, { linkError: "Handle is required", linkPid: pid });

    const { api } = locals;
    const res = await api.post(
      `/projects/${params.id}/players/${pid}/accounts`,
      { handle },
    );
    if (!res.ok) {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to link account" }));
      return fail(res.status, { linkError: err.message, linkPid: pid });
    }
  },

  unlinkAccount: async ({ request, params, locals }) => {
    const data = await request.formData();
    const pid = data.get("pid") as string;
    const aid = data.get("aid") as string;
    const { api } = locals;
    const res = await api.delete(
      `/projects/${params.id}/players/${pid}/accounts/${aid}`,
    );
    if (!res.ok)
      return fail(res.status, { deleteError: "Failed to unlink account" });
  },
};
