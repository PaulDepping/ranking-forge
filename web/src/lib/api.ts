import { env } from "$env/dynamic/public";

export function makeApi(fetchFn: typeof fetch) {
  async function req(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<Response> {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["Content-Type"] = "application/json";
    return fetchFn(env.PUBLIC_API_URL + path, {
      method,
      credentials: "include",
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
    putRanking: (projectId: string, playerIds: string[]) =>
      req("PUT", `/projects/${projectId}/ranking`, { player_ids: playerIds }),
  };
}
