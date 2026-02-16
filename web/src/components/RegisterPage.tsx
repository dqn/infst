import type { FC } from "hono/jsx";
import { Layout } from "./Layout";

interface RegisterPageProps {
  error?: string;
  values?: { email?: string; username?: string };
}

export const RegisterPage: FC<RegisterPageProps> = ({ error, values }) => {
  return (
    <Layout title="Register">
      <div style="max-width:400px;margin:48px auto;">
        <h2 style="margin-bottom:24px;">Create an account</h2>
        <div class="card">
          {error ? <p class="error">{error}</p> : null}
          <form method="post" action="/auth/register">
            <div style="margin-bottom:12px;">
              <label style="display:block;font-size:0.85rem;color:#999;margin-bottom:4px;">Email</label>
              <input
                type="email"
                name="email"
                placeholder="you@example.com"
                required
                value={values?.email ?? ""}
                style="width:100%;"
              />
            </div>
            <div style="margin-bottom:12px;">
              <label style="display:block;font-size:0.85rem;color:#999;margin-bottom:4px;">Username</label>
              <input
                type="text"
                name="username"
                placeholder="username"
                required
                pattern="[a-z0-9_\-]{3,20}"
                value={values?.username ?? ""}
                style="width:100%;"
              />
              <p style="font-size:0.8rem;color:#666;margin-top:4px;">
                3-20 characters. Lowercase letters, numbers, hyphens, underscores.
              </p>
            </div>
            <div style="margin-bottom:12px;">
              <label style="display:block;font-size:0.85rem;color:#999;margin-bottom:4px;">Password</label>
              <input
                type="password"
                name="password"
                placeholder="Password"
                required
                minLength={8}
                maxLength={72}
                style="width:100%;"
              />
              <p style="font-size:0.8rem;color:#666;margin-top:4px;">
                8-72 characters.
              </p>
            </div>
            <button type="submit" style="width:100%;">Register</button>
          </form>
        </div>
        <p style="margin-top:16px;text-align:center;color:#999;">
          Already have an account? <a href="/login">Login</a>
        </p>
      </div>
    </Layout>
  );
};
