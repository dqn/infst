import type { FC } from "hono/jsx";
import { Layout } from "./Layout";

interface SettingsPageProps {
  user: {
    username: string | null;
    apiToken: string | null;
    isPublic: boolean;
  };
}

export const SettingsPage: FC<SettingsPageProps> = ({ user }) => {
  return (
    <Layout title="Settings" user={user}>
      <div style="max-width:500px;margin:24px auto;">
        <h2 style="margin-bottom:24px;">Settings</h2>

        {/* API Token */}
        <div class="card" style="margin-bottom:16px;">
          <h3 style="margin-bottom:12px;font-size:1rem;">API Token</h3>
          <div style="display:flex;gap:8px;align-items:center;margin-bottom:8px;">
            <input
              type="text"
              id="api-token"
              value={user.apiToken ?? ""}
              readonly
              style="flex:1;font-family:monospace;font-size:0.85rem;"
            />
            <button type="button" id="copy-token" style="white-space:nowrap;">
              Copy
            </button>
          </div>
          <button type="button" id="regen-token" class="danger" style="font-size:0.85rem;">
            Regenerate
          </button>
        </div>

        {/* Visibility */}
        <div class="card" style="margin-bottom:16px;">
          <h3 style="margin-bottom:12px;font-size:1rem;">Profile Visibility</h3>
          <label style="display:flex;align-items:center;gap:8px;cursor:pointer;">
            <input
              type="checkbox"
              id="is-public"
              checked={user.isPublic}
            />
            <span>Public profile</span>
          </label>
          <p style="font-size:0.8rem;color:#666;margin-top:4px;">
            When disabled, your lamp data will not be visible to others.
          </p>
        </div>

        <script src="/settings.js"></script>
      </div>
    </Layout>
  );
};
