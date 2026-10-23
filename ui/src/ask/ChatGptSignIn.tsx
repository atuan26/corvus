import { useEffect, useState } from "react";
import { Button } from "react-aria-components";
import { api, type ChatGptStatus, type DeviceCode } from "../api";

type Phase = { kind: "idle" } | { kind: "browser" } | { kind: "device"; code: DeviceCode | null };

/** Sign in / out of ChatGPT, by browser or by code. `onChange` fires after the state changes. */
export function ChatGptSignIn({ onChange, compact }: { onChange?: () => void; compact?: boolean }) {
  const [status, setStatus] = useState<ChatGptStatus | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: "idle" });
  const [message, setMessage] = useState("");

  const refresh = () => api.chatgptStatus().then(setStatus).catch((e) => setMessage(String(e)));
  useEffect(() => void refresh(), []);

  const done = async (p: Promise<ChatGptStatus>) => {
    try {
      const s = await p;
      setStatus(s);
      setMessage(`Signed in${s.email ? ` as ${s.email}` : ""}`);
      onChange?.();
    } catch (e) {
      const text = String(e);
      setMessage(text.includes("cancelled") ? "Sign-in cancelled" : `Sign-in failed: ${text}`);
    } finally {
      setPhase({ kind: "idle" });
    }
  };

  const browser = () => {
    setMessage("");
    setPhase({ kind: "browser" });
    void done(api.chatgptSignIn());
  };

  const device = async () => {
    setMessage("");
    setPhase({ kind: "device", code: null });
    try {
      const code = await api.chatgptDeviceStart();
      setPhase({ kind: "device", code });
      void done(api.chatgptDeviceFinish());
    } catch (e) {
      setPhase({ kind: "idle" });
      setMessage(`Could not start: ${String(e)}`);
    }
  };

  const cancel = () => void api.chatgptCancelSignIn();

  const signOut = async () => {
    await api.chatgptSignOut();
    await refresh();
    setMessage("Signed out");
    onChange?.();
  };

  if (!status) return <p className="setting-hint">Checking sign-in…</p>;

  return (
    <div className="signin">
      {status.signed_in ? (
        <p className="signin-status">
          Signed in{status.email ? ` as ${status.email}` : ""}
          {status.plan ? ` (${status.plan} plan)` : ""}.
          {!compact && (
            <span className="setting-hint">
              {" "}
              Kept in {status.storage === "keychain" ? "the system keychain" : "a private file readable only by you"}.
            </span>
          )}
        </p>
      ) : (
        !compact && <p className="signin-status">Use your ChatGPT plan instead of an API key.</p>
      )}

      {phase.kind === "browser" && (
        <p className="notice" role="status">
          Finish signing in in your browser. <Button className="btn" onPress={cancel}>Cancel</Button>
        </p>
      )}
      {phase.kind === "device" && (
        <div className="notice" role="status">
          {phase.code ? (
            <>
              <p>
                On any device, open <code>{phase.code.verification_url}</code> and enter
              </p>
              <p className="device-code" aria-label={`Code ${phase.code.user_code.split("").join(" ")}`}>
                {phase.code.user_code}
              </p>
              <div className="signin-actions">
                <Button className="btn" onPress={() => void api.openUrl(phase.code!.verification_url)}>
                  Open the page
                </Button>
                <Button className="btn" onPress={() => void navigator.clipboard.writeText(phase.code!.user_code)}>
                  Copy code
                </Button>
                <Button className="btn" onPress={cancel}>
                  Cancel
                </Button>
              </div>
            </>
          ) : (
            "Getting a code…"
          )}
        </div>
      )}

      {phase.kind === "idle" && (
        <div className="signin-actions">
          {status.signed_in ? (
            <Button className="btn" onPress={() => void signOut()}>
              Sign out
            </Button>
          ) : (
            <>
              <Button className="btn btn-primary" onPress={browser}>
                Sign in with ChatGPT
              </Button>
              <Button className="btn" onPress={() => void device()}>
                Sign in with a code
              </Button>
            </>
          )}
        </div>
      )}
      {message && (
        <p className="setting-hint" role="status">
          {message}
        </p>
      )}
    </div>
  );
}
