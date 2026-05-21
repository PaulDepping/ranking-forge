import { describe, it, expect, vi } from "vitest";
import { makeServerApi } from "./api";

describe("makeServerApi", () => {
  it("forwards session cookie when sessionId is provided", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response());
    const api = makeServerApi(mockFetch as typeof fetch, "abc123");
    await api.get("/test");
    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:8080/test",
      expect.objectContaining({
        headers: expect.objectContaining({ Cookie: "session_id=abc123" }),
      }),
    );
  });

  it("omits Cookie header when sessionId is undefined", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response());
    const api = makeServerApi(mockFetch as typeof fetch, undefined);
    await api.get("/test");
    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:8080/test",
      expect.objectContaining({
        headers: expect.not.objectContaining({ Cookie: expect.anything() }),
      }),
    );
  });

  it("sends POST with JSON body and Content-Type header", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response());
    const api = makeServerApi(mockFetch as typeof fetch, undefined);
    await api.post("/test", { foo: "bar" });
    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:8080/test",
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({
          "Content-Type": "application/json",
        }),
        body: JSON.stringify({ foo: "bar" }),
      }),
    );
  });

  it("sends DELETE with no body", async () => {
    const mockFetch = vi.fn().mockResolvedValue(new Response());
    const api = makeServerApi(mockFetch as typeof fetch, undefined);
    await api.delete("/test");
    expect(mockFetch).toHaveBeenCalledWith(
      "http://localhost:8080/test",
      expect.objectContaining({ method: "DELETE", body: undefined }),
    );
  });

  it("returns the raw Response from fetch", async () => {
    const fakeResponse = new Response(JSON.stringify({ ok: true }), {
      status: 200,
    });
    const mockFetch = vi.fn().mockResolvedValue(fakeResponse);
    const api = makeServerApi(mockFetch as typeof fetch, "tok");
    const result = await api.get("/me");
    expect(result).toBe(fakeResponse);
  });
});
