import type { Actions, PageServerLoad } from "./$types";
import { redirect } from "@sveltejs/kit";
import { env } from "$env/dynamic/private";

export const load: PageServerLoad = () => {
  redirect(303, "/");
};

export const actions: Actions = {
  default: async ({ fetch, cookies }) => {
    const sessionId = cookies.get("session_id");
    if (sessionId) {
      await fetch(`${env.INTERNAL_API_URL}/auth/logout`, {
        method: "POST",
        headers: { Cookie: `session_id=${sessionId}` },
      }).catch(() => {});
    }
    cookies.delete("session_id", { path: "/" });
    redirect(303, "/login");
  },
};
