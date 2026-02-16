import { createMiddleware } from "hono/factory";
import { getCookie } from "hono/cookie";
import { eq } from "drizzle-orm";

import type { AppEnv } from "../lib/types";
import { verifyJwt, signJwt } from "../lib/token";
import { users } from "../db/schema";
import { SESSION_MAX_AGE_SECONDS } from "../lib/constants";

// Session authentication middleware using JWT cookies
export const sessionAuth = createMiddleware<AppEnv>(async (c, next) => {
  const token = getCookie(c, "session");
  if (!token) {
    return c.redirect("/login");
  }

  const payload = await verifyJwt(token, c.env.JWT_SECRET);
  if (!payload || typeof payload.userId !== "number") {
    return c.redirect("/login");
  }

  const db = c.get("db");
  const result = await db
    .select()
    .from(users)
    .where(eq(users.id, payload.userId as number))
    .limit(1);

  const user = result[0];
  if (!user) {
    return c.redirect("/login");
  }

  c.set("user", user);
  await next();
});

// Optional session middleware that does not redirect on failure
export const optionalSession = createMiddleware<AppEnv>(async (c, next) => {
  const token = getCookie(c, "session");
  if (!token) {
    c.set("user", null);
    await next();
    return;
  }

  const payload = await verifyJwt(token, c.env.JWT_SECRET);
  if (!payload || typeof payload.userId !== "number") {
    c.set("user", null);
    await next();
    return;
  }

  const db = c.get("db");
  const result = await db
    .select()
    .from(users)
    .where(eq(users.id, payload.userId as number))
    .limit(1);

  c.set("user", result[0] ?? null);
  await next();
});

// Create a session cookie with JWT
export async function createSessionCookie(
  userId: number,
  secret: string,
): Promise<string> {
  return signJwt({ userId, iat: Math.floor(Date.now() / 1000) }, secret);
}

// Set session cookie on the response
export function setSessionCookie(
  c: { header: (name: string, value: string) => void },
  token: string,
): void {
  const cookie = `session=${token}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=${SESSION_MAX_AGE_SECONDS}`;
  c.header("Set-Cookie", cookie);
}
