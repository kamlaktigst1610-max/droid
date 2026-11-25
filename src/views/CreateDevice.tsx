import { useEffect, useMemo, useState } from "react";
import { api, errorText } from "../api";
import type { DeviceProfile, SdkInfo } from "../types";

interface Props {
  sdk: SdkInfo | null;
  profiles: DeviceProfile[];
  existing: string[];
  onCreated: () => void | Promise<void>;
  onRescan: () => void;
}

const RAM_OPTIONS = [1024, 1536, 2048, 3072, 4096];

export default function CreateDevice({ sdk, profiles, existing, onCreated, onRescan }: Props) {
  const images = sdk?.system_images ?? [];
  const [profileId, setProfileId] = useState("lite");
  const [imageId, setImageId] = useState("");
  const [name, setName] = useState("droidlite-lite");
  const [nameTouched, setNameTouched] = useState(false);
  const [cpus, setCpus] = useState(2);
  const [ram, setRam] = useState(1536);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const profile = useMemo(
    () => profiles.find((p) => p.id === profileId) ?? profiles[0],
    [profiles, profileId],
  );

  // Pick the default image once the SDK scan arrives (first entry is the best match).
  useEffect(() => {
    if (!imageId && images.length > 0) setImageId(images[0].id);
  }, [images, imageId]);

  // Applying a profile resets the editable fields.
  useEffect(() => {
    if (!profile) return;
    setCpus(profile.cpus);
    setRam(profile.ram_mb);
    if (!nameTouched) setName(`droidlite-${profile.id}`);
  }, [profile, nameTouched]);

  const image = images.find((i) => i.id === imageId);
  const nameClash = existing.includes(name);
  const nameValid = /^[A-Za-z0-9_.-]{1,40}$/.test(name);
  const canCreate = !!profile && !!image && nameValid && !nameClash && !busy;
  const ramChoices = RAM_OPTIONS.includes(ram) ? RAM_OPTIONS : [...RAM_OPTIONS, ram].sort((a, b) => a - b);

  async function submit() {
    if (!profile || !image) return;
    setBusy(true);
    setError(null);
    try {
      await api.createDevice({
        name,
        image_id: image.id,
        cpus,
        ram_mb: ram,
        width: profile.width,
        height: profile.height,
        density: profile.density,
      });
      await onCreated();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  if (sdk && images.length === 0) {
    return (
      <section>
        <header className="page-head">
          <h1>Create Device</h1>
        </header>
        <div className="banner warn">
          <div>
            <strong>No system images found</strong>
            <p>
              DroidLite doesn’t bundle Android system images. Install one with the SDK
              command-line tools, then scan again:
            </p>
            <pre className="code">sdkmanager "system-images;android-30;default;x86_64"</pre>
          </div>
          <button onClick={onRescan}>Scan again</button>
        </div>
      </section>
    );
  }

  return (
    <section>
      <header className="page-head">
        <h1>Create Device</h1>
      </header>

      <div className="form">
        <label className="field">
          <span>Name</span>
          <input
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setNameTouched(true);
            }}
            spellCheck={false}
          />
          {!nameValid && <small className="bad">Use letters, numbers, dots, dashes or underscores.</small>}
          {nameClash && <small className="bad">A device with this name already exists.</small>}
        </label>

        <fieldset className="field">
          <legend>Device profile</legend>
          <div className="profiles">
            {profiles.map((p) => (
              <label key={p.id} className={p.id === profileId ? "profile on" : "profile"}>
                <input
                  type="radio"
                  name="profile"
                  checked={p.id === profileId}
                  onChange={() => setProfileId(p.id)}
                />
                <span className="p-title">{p.label}</span>
                <span className="p-spec">
                  {p.width}×{p.height}, {p.cpus} CPU, {p.ram_mb} MB
                </span>
                <span className="muted small">{p.description}</span>
              </label>
            ))}
          </div>
        </fieldset>

        <label className="field">
          <span>Android system image</span>
          <select value={imageId} onChange={(e) => setImageId(e.target.value)}>
            {images.map((i) => (
              <option key={i.id} value={i.id}>
                {i.android} (API {i.api}), {i.variant.replace(/_/g, " ")}, {i.abi}
                {i.recommended ? " – recommended" : ""}
              </option>
            ))}
          </select>
          {image && image.variant.includes("playstore") && (
            <small className="muted">Google Play images use more memory and disk.</small>
          )}
          {image && image.abi !== "x86_64" && (
            <small className="bad">Only x86_64 images are hardware accelerated on Windows.</small>
          )}
        </label>

        <div className="row">
          <label className="field">
            <span>CPU cores</span>
            <select value={cpus} onChange={(e) => setCpus(Number(e.target.value))}>
              {[1, 2, 3, 4].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span>RAM</span>
            <select value={ram} onChange={(e) => setRam(Number(e.target.value))}>
              {ramChoices.map((n) => (
                <option key={n} value={n}>
                  {n} MB
                </option>
              ))}
            </select>
          </label>
          <div className="field">
            <span>Screen</span>
            <div className="static">
              {profile ? `${profile.width}×${profile.height}, ${profile.density} dpi` : "–"}
            </div>
          </div>
        </div>

        {error && (
          <div className="banner error" role="alert">
            <span>{error}</span>
          </div>
        )}

        <div className="form-actions">
          <button className="primary" disabled={!canCreate} onClick={submit}>
            {busy ? "Creating…" : "Create device"}
          </button>
        </div>
      </div>
    </section>
  );
}
