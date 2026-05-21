import { env } from "$env/dynamic/private";

export function makeServerApi(
  fetchFn: typeof fetch,
  sessionId: string | undefined,
) {
  async function req(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<Response> {
    const headers: Record<string, string> = {};
    if (sessionId) headers["Cookie"] = `session_id=${sessionId}`;
    if (body !== undefined) headers["Content-Type"] = "application/json";
    return fetchFn(env.INTERNAL_API_URL + path, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  }

  return {
    get: (path: string) => req("GET", path),
    post: (path: string, body?: unknown) => req("POST", path, body),
    patch: (path: string, body: unknown) => req("PATCH", path, body),
    put: (path: string, body: unknown) => req("PUT", path, body),
    delete: (path: string) => req("DELETE", path),
  };
}

export type ServerApi = ReturnType<typeof makeServerApi>;
