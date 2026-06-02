import { makeServerApi } from "$lib/server/api";
import { redirect } from "@sveltejs/kit";
import { env } from "$env/dynamic/private";
import type { Handle } from "@sveltejs/kit";

export const handle: Handle = async ({ event, resolve }) => {
  const { pathname } = event.url;

  const sessionId = event.cookies.get("session_id");
  event.locals.api = makeServerApi(event.fetch, sessionId);

  if (sessionId) {
    const res = await event.locals.api.get("/auth/me");
    if (res.ok) {
      event.locals.user = await res.json();
    } else {
      event.cookies.delete("session_id", {
        path: "/",
        ...(env.COOKIE_DOMAIN ? { domain: env.COOKIE_DOMAIN } : {}),
      });
      event.locals.user = null;
    }
  } else {
    event.locals.user = null;
  }

  if (!event.locals.user) {
    const isPublic =
      pathname === "/" ||
      ["/login", "/register", "/logout"].includes(pathname) ||
      /^\/projects\/[^/]/.test(pathname) ||
      /^\/invite\//.test(pathname);
    if (!isPublic) {
      redirect(303, "/login");
    }
  }

  return resolve(event);
};
