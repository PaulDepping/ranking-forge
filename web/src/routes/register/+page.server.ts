import { fail, redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import { env } from "$env/dynamic/private";

export const load: PageServerLoad = ({ locals }) => {
  if (locals.user) redirect(303, "/projects");
};

export const actions: Actions = {
  default: async ({ fetch, request, cookies }) => {
    const data = await request.formData();
    const email = data.get("email") as string;
    const display_name = data.get("display_name") as string;
    const password = data.get("password") as string;
    const confirmPassword = data.get("confirm_password") as string;

    if (password !== confirmPassword) {
      return fail(400, { error: "Passwords do not match" });
    }

    const res = await fetch(`${env.INTERNAL_API_URL}/auth/register`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, display_name, password }),
    });

    if (!res.ok) {
      const body = await res
        .json()
        .catch(() => ({ message: "Registration failed" }));
      return fail(res.status, { error: body.message ?? "Registration failed" });
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

    redirect(303, "/projects");
  },
};
