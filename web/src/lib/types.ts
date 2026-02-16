import type { DrizzleD1Database } from "drizzle-orm/d1";

export interface Env {
  DB: D1Database;
  JWT_SECRET: string;
  ADMIN_TOKEN: string;
  APP_URL: string;
}

export interface SessionUser {
  id: number;
  email: string;
  username: string;
  apiToken: string | null;
  apiTokenCreatedAt: string | null;
  isPublic: boolean;
}

export interface AuthUser {
  id: number;
  email: string;
  username: string | null;
  apiToken: string | null;
  apiTokenCreatedAt: string | null;
  isPublic: boolean;
}

export interface Variables {
  db: DrizzleD1Database;
  user: SessionUser | AuthUser | null;
}

export interface AppEnv {
  Bindings: Env;
  Variables: Variables;
}
