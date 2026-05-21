import { makeServerApi } from "$lib/server/api";
import { redirect } from "@sveltejs/kit";
import type { Handle } from "@sveltejs/kit";

export const handle: Handle = async ({ event, resolve }) => {
  const { pathname } = event.url;

  const sessionId = event.cookies.get("session_id");
  event.locals.api = makeServerApi(event.fetch, sessionId);

  const res = await event.locals.api.get("/auth/me");
  if (res.ok) {
    event.locals.user = await res.json();
  } else {
    event.locals.user = null;
    const isPublic =
      pathname === "/" ||
      ["/login", "/register"].includes(pathname) ||
      /^\/projects\/[^/]/.test(pathname) ||
      /^\/invite\//.test(pathname);
    if (!isPublic) {
      redirect(303, "/login");
    }
  }

  return resolve(event);
};
