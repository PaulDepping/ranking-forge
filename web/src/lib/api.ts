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
    putRanking: (projectId: string, rankingId: string, playerIds: string[]) =>
      req("PUT", `/projects/${projectId}/rankings/${rankingId}/ranking`, {
        player_ids: playerIds,
      }),
    createRanking: (projectId: string, name: string, description?: string) =>
      req("POST", `/projects/${projectId}/rankings`, { name, description }),
    patchRanking: (
      projectId: string,
      rankingId: string,
      body: { name?: string; description?: string; published?: boolean },
    ) => req("PATCH", `/projects/${projectId}/rankings/${rankingId}`, body),
    deleteRanking: (projectId: string, rankingId: string) =>
      req("DELETE", `/projects/${projectId}/rankings/${rankingId}`),
    addRankingPlayer: (
      projectId: string,
      rankingId: string,
      playerId: string,
    ) =>
      req("POST", `/projects/${projectId}/rankings/${rankingId}/players`, {
        player_id: playerId,
      }),
    removeRankingPlayer: (
      projectId: string,
      rankingId: string,
      playerId: string,
    ) =>
      req(
        "DELETE",
        `/projects/${projectId}/rankings/${rankingId}/players/${playerId}`,
      ),
    patchRankingPlayer: (
      projectId: string,
      rankingId: string,
      playerId: string,
      notes: string | null,
    ) =>
      req(
        "PATCH",
        `/projects/${projectId}/rankings/${rankingId}/players/${playerId}`,
        { notes },
      ),
  };
}
