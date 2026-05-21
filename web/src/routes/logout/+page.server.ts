import type { Actions, PageServerLoad } from "./$types";
import { redirect } from "@sveltejs/kit";
import { env } from "$env/dynamic/private";

export const load: PageServerLoad = () => {
  redirect(303, "/");
};

export const actions: Actions = {
  default: async ({ locals, cookies }) => {
    if (locals.api) {
      await locals.api.post("/auth/logout").catch(() => {});
    }
    cookies.delete("session_id", {
      path: "/",
      ...(env.COOKIE_DOMAIN ? { domain: env.COOKIE_DOMAIN } : {}),
    });
    redirect(303, "/login");
  },
};
