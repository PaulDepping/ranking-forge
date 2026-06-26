import { fail, redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";

export const load: PageServerLoad = ({ locals }) => {
  if (!locals.user) redirect(303, "/login");
  return { user: locals.user };
};

export const actions: Actions = {
  updateProfile: async ({ request, locals }) => {
    if (!locals.user) return fail(401, { error: "Unauthorized" });
    const data = await request.formData();
    const display_name = data.get("display_name") as string | null;
    const email = data.get("email") as string | null;

    const body: Record<string, string> = {};
    if (display_name) body.display_name = display_name;
    if (email) body.email = email;

    if (Object.keys(body).length === 0) {
      return fail(422, {
        profileError: "Provide at least one field to update.",
      });
    }

    const { api } = locals;
    const res = await api.patch("/account/profile", body);

    if (!res.ok) {
      const json = await res.json().catch(() => ({ message: "Update failed" }));
      return fail(res.status, {
        profileError: json.message ?? "Update failed",
      });
    }

    return { profileSuccess: true };
  },

  updatePassword: async ({ request, locals }) => {
    if (!locals.user) return fail(401, { error: "Unauthorized" });
    const data = await request.formData();
    const current_password = data.get("current_password") as string;
    const new_password = data.get("new_password") as string;
    const confirm_password = data.get("confirm_password") as string;

    if (new_password !== confirm_password) {
      return fail(400, { passwordError: "New passwords do not match." });
    }

    const { api } = locals;
    const res = await api.patch("/account/password", {
      current_password,
      new_password,
    });

    if (!res.ok) {
      const json = await res
        .json()
        .catch(() => ({ message: "Password change failed" }));
      return fail(res.status, {
        passwordError: json.message ?? "Password change failed",
      });
    }

    return { passwordSuccess: true };
  },

  deleteAccount: async ({ locals }) => {
    if (!locals.user) return fail(401, { error: "Unauthorized" });

    const { api } = locals;
    const res = await api.delete("/account");

    if (!res.ok) {
      const json = await res.json().catch(() => ({ message: "Delete failed" }));
      return fail(res.status, { deleteError: json.message ?? "Delete failed" });
    }

    redirect(303, "/login");
  },
};
