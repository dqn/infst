import type { FC } from "hono/jsx";
import { Layout } from "./Layout";

interface LoginPageProps {
  error?: string;
  values?: { email?: string };
}

export const LoginPage: FC<LoginPageProps> = ({ error, values }) => {
  return (
    <Layout title="Login">
      <div style="max-width:400px;margin:48px auto;">
        <h2 style="margin-bottom:24px;">Login</h2>
        <div class="card">
          {error ? <p class="error">{error}</p> : null}
          <form method="post" action="/auth/login">
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
            </div>
            <button type="submit" style="width:100%;">Login</button>
          </form>
        </div>
        <p style="margin-top:16px;text-align:center;color:#999;">
          Don't have an account? <a href="/register">Register</a>
        </p>
      </div>
    </Layout>
  );
};
