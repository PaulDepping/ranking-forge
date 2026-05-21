import { fail, redirect, error } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import type { ProjectMember, InviteLink } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals, parent }) => {
  const { project } = await parent();
  if (project.user_role !== "owner") {
    error(403, { message: "forbidden" });
  }

  const { api } = locals;
  const [membersRes, linksRes] = await Promise.all([
    api.get(`/projects/${params.id}/members`),
    api.get(`/projects/${params.id}/invite-links`),
  ]);

  const members: ProjectMember[] = membersRes.ok ? await membersRes.json() : [];
  const inviteLinks: InviteLink[] = linksRes.ok ? await linksRes.json() : [];

  return { members, inviteLinks };
};

export const actions: Actions = {
  rename: async ({ params, locals, request }) => {
    const data = await request.formData();
    const name = ((data.get("name") as string) ?? "").trim();
    if (!name) return fail(400, { renameError: "Name is required" });
    if ([...name].length > 100)
      return fail(400, { renameError: "Name must be at most 100 characters" });
    const { api } = locals;
    const res = await api.patch(`/projects/${params.id}`, { name });
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: "Rename failed" }));
      return fail(res.status, { renameError: body.message });
    }
    return { project: await res.json() };
  },

  publish: async ({ params, locals, request }) => {
    const data = await request.formData();
    const published = data.get("published") === "true";
    const { api } = locals;
    const res = await api.patch(`/projects/${params.id}`, { published });
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to update" }));
      return fail(res.status, { publishError: body.message });
    }
    return { project: await res.json() };
  },

  addMember: async ({ params, locals, request }) => {
    const data = await request.formData();
    const email = ((data.get("email") as string) ?? "").trim();
    const role = data.get("role") as string;
    if (!email) return fail(400, { memberError: "Email is required" });
    if (!["editor", "viewer"].includes(role))
      return fail(400, { memberError: "Invalid role" });
    const { api } = locals;
    const res = await api.post(`/projects/${params.id}/members`, {
      email,
      role,
    });
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to add member" }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  removeMember: async ({ params, locals, request }) => {
    const data = await request.formData();
    const userId = data.get("user_id") as string;
    const { api } = locals;
    const res = await api.delete(`/projects/${params.id}/members/${userId}`);
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to remove member" }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  changeMemberRole: async ({ params, locals, request }) => {
    const data = await request.formData();
    const userId = data.get("user_id") as string;
    const role = data.get("role") as string;
    const { api } = locals;
    const res = await api.patch(`/projects/${params.id}/members/${userId}`, {
      role,
    });
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to update role" }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  transferOwnership: async ({ params, locals, request }) => {
    const data = await request.formData();
    const userId = data.get("user_id") as string;
    const { api } = locals;
    const res = await api.post(
      `/projects/${params.id}/members/transfer-ownership`,
      {
        user_id: userId,
      },
    );
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Transfer failed" }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  createInviteLink: async ({ params, locals, request }) => {
    const data = await request.formData();
    const role = data.get("role") as string;
    const expiresAtRaw = data.get("expires_at") as string | null;
    const expires_at = expiresAtRaw
      ? new Date(expiresAtRaw).toISOString()
      : undefined;
    if (!["editor", "viewer"].includes(role))
      return fail(400, { linkError: "Invalid role" });
    const { api } = locals;
    const res = await api.post(`/projects/${params.id}/invite-links`, {
      role,
      expires_at,
    });
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to create link" }));
      return fail(res.status, { linkError: body.message });
    }
    return { newLink: await res.json() };
  },

  revokeInviteLink: async ({ params, locals, request }) => {
    const data = await request.formData();
    const linkId = data.get("link_id") as string;
    const { api } = locals;
    const res = await api.delete(
      `/projects/${params.id}/invite-links/${linkId}`,
    );
    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Failed to revoke link" }));
      return fail(res.status, { linkError: body.message });
    }
    return {};
  },

  delete: async ({ params, locals }) => {
    const { api } = locals;
    const res = await api.delete(`/projects/${params.id}`);
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: "Delete failed" }));
      return fail(res.status, { deleteError: body.message });
    }
    redirect(303, "/projects");
  },
};
