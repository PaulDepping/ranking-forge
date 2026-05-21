import { fail, redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import { env } from "$env/dynamic/private";

export const load: PageServerLoad = ({ locals, url }) => {
  if (locals.user) redirect(303, "/projects");
  const redirectTo = url.searchParams.get("redirect") ?? "/projects";
  return { redirectTo };
};

export const actions: Actions = {
  default: async ({ fetch, request, cookies }) => {
    const data = await request.formData();
    const email = data.get("email") as string;
    const password = data.get("password") as string;
    const redirectTo = (data.get("redirect") as string) ?? "/projects";

    const res = await fetch(`${env.INTERNAL_API_URL}/auth/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password }),
    });

    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: "Login failed" }));
      return fail(res.status, { error: body.message ?? "Login failed" });
    }

    const setCookie = res.headers.get("set-cookie");
    const match = setCookie?.match(/session_id=([^;]+)/);
    if (match) {
      cookies.set("session_id", match[1], {
        path: "/",
        httpOnly: true,
        sameSite: "strict",
        maxAge: 60 * 60 * 24 * 30,
        ...(env.COOKIE_DOMAIN ? { domain: env.COOKIE_DOMAIN } : {}),
      });
    }

    const safe =
      redirectTo.startsWith("/") && !redirectTo.startsWith("//")
        ? redirectTo
        : "/projects";
    redirect(303, safe);
  },
};
