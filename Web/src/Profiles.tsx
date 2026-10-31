import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Copy, Plus, SlidersHorizontal, Trash2, X } from "lucide-react";
import { useAuth } from "./auth";
import { Modal } from "./Modal";
import { useWorkspace } from "./data";
import { uuid, type Profile, type Format } from "./models";
import { Empty, ErrorNote, Field, Loading } from "./ui";

export function Profiles() {
  const { api } = useAuth();
  const queries = useQueryClient();
  const { profiles } = useWorkspace();
  const [editing, setEditing] = useState<{
    profile: Profile;
    create: boolean;
  }>();
  const [deleting, setDeleting] = useState<Profile>();
  const remove = useMutation({
    mutationFn: (id: string) => api.send(`profiles/${id}`, "DELETE"),
    onSuccess: () => {
      setDeleting(undefined);
      void queries.invalidateQueries({ queryKey: ["profiles"] });
    },
  });
  return (
    <>
      <header className="page-heading">
        <div>
          <p className="eyebrow">CONSISTENT CAPTURE SETTINGS</p>
          <h1>Scan profiles</h1>
          <p>Save a setup once. Use it from any connected interface.</p>
        </div>
        <button
          className="button primary"
          onClick={() =>
            setEditing({
              profile: {
                id: uuid(),
                name: "",
                dpi: 300,
                duplex: true,
                format: "png",
              },
              create: true,
            })
          }
        >
          <Plus size={17} />
          New profile
        </button>
      </header>
      <p className="library-note">
        Profiles currently use A4, RGB colour and 8-bit channels. A Pokémon
        profile name does not mean card feeding has been validated.
      </p>
      <ErrorNote error={profiles.error || remove.error} />
      {profiles.isPending ? (
        <Loading />
      ) : !profiles.data?.profiles.length ? (
        <Empty title="Make your first profile">
          Save your preferred resolution, sides and file format.
        </Empty>
      ) : (
        <div className="profile-grid">
          {profiles.data.profiles.map((profile) => (
            <article key={profile.id} className="panel profile-card">
              <span className="profile-symbol">
                <SlidersHorizontal size={23} />
              </span>
              <h2>{profile.name}</h2>
              <p>
                {profile.dpi} DPI · {profile.duplex ? "Both sides" : "One side"}{" "}
                · {profile.format.toUpperCase()}
              </p>
              <div className="profile-actions">
                <button
                  className="button secondary"
                  onClick={() => setEditing({ profile, create: false })}
                >
                  Edit profile
                </button>
                <button
                  className="icon-button"
                  aria-label={`Duplicate ${profile.name}`}
                  onClick={() =>
                    setEditing({
                      profile: {
                        ...profile,
                        id: uuid(),
                        name: profile.name + " copy",
                      },
                      create: true,
                    })
                  }
                >
                  <Copy size={17} />
                </button>
                <button
                  className="icon-button"
                  aria-label={`Delete ${profile.name}`}
                  onClick={() => setDeleting(profile)}
                >
                  <Trash2 size={17} />
                </button>
              </div>
            </article>
          ))}
        </div>
      )}
      {editing && (
        <ProfileEditor
          key={editing.profile.id}
          {...editing}
          close={() => setEditing(undefined)}
        />
      )}
      {deleting && (
        <Modal
          close={() => {
            if (!remove.isPending) setDeleting(undefined);
          }}
        >
          <section
            className="modal compact"
            role="dialog"
            aria-modal="true"
            aria-labelledby="delete-title"
          >
            <h2 id="delete-title">Delete {deleting.name}?</h2>
            <p>Existing jobs keep their saved settings and images.</p>
            <div className="toolbar">
              <button
                autoFocus
                className="button secondary"
                onClick={() => setDeleting(undefined)}
              >
                Keep profile
              </button>
              <button
                className="button danger"
                disabled={remove.isPending}
                onClick={() => remove.mutate(deleting.id)}
              >
                Delete profile
              </button>
            </div>
            <ErrorNote error={remove.error} />
          </section>
        </Modal>
      )}
    </>
  );
}
function ProfileEditor({
  profile,
  create,
  close,
}: {
  profile: Profile;
  create: boolean;
  close: () => void;
}) {
  const { api } = useAuth();
  const queries = useQueryClient();
  const [value, setValue] = useState(profile);
  const save = useMutation({
    mutationFn: () => {
      const { id, ...fields } = value;
      return api.send(
        create ? "profiles" : `profiles/${id}`,
        create ? "POST" : "PATCH",
        create ? value : fields,
      );
    },
    onSuccess: () => {
      void queries.invalidateQueries({ queryKey: ["profiles"] });
      close();
    },
  });
  return (
    <Modal
      close={() => {
        if (!save.isPending) close();
      }}
    >
      <form
        className="modal compact"
        role="dialog"
        aria-modal="true"
        aria-labelledby="profile-title"
        onSubmit={(e) => {
          e.preventDefault();
          save.mutate();
        }}
      >
        <div className="section-heading">
          <h2 id="profile-title">{create ? "New" : "Edit"} profile</h2>
          <button
            type="button"
            className="icon-button"
            aria-label="Close profile editor"
            onClick={close}
          >
            <X />
          </button>
        </div>
        <fieldset className="form-stack" disabled={save.isPending}>
          <Field label="Profile name">
            <input
              autoFocus
              value={value.name}
              required
              maxLength={128}
              onChange={(e) => setValue({ ...value, name: e.target.value })}
            />
          </Field>
          <div className="form-row">
            <Field label="Resolution (DPI)">
              <input
                type="number"
                min={150}
                max={600}
                step={1}
                required
                value={value.dpi}
                onChange={(e) =>
                  setValue({ ...value, dpi: Number(e.target.value) })
                }
              />
            </Field>
            <Field label="Format">
              <select
                value={value.format}
                onChange={(e) =>
                  setValue({ ...value, format: e.target.value as Format })
                }
              >
                <option value="png">PNG</option>
                <option value="tiff">TIFF</option>
              </select>
            </Field>
          </div>
          <label className="toggle-row">
            <span>Scan both sides</span>
            <input
              type="checkbox"
              checked={value.duplex}
              onChange={(e) => setValue({ ...value, duplex: e.target.checked })}
            />
          </label>
          <p className="small muted">
            The selected scanner must support this resolution. It is checked
            before scanning.
          </p>
          <ErrorNote error={save.error} />
          <button className="button primary full" disabled={!value.name.trim()}>
            {save.isPending ? "Saving…" : "Save profile"}
          </button>
        </fieldset>
      </form>
    </Modal>
  );
}
