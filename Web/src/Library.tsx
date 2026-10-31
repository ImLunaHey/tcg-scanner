import { useEffect, useState } from "react";
import {
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { Link } from "wouter";
import {
  ArrowLeft,
  Download,
  ImageIcon,
  Images,
  SlidersHorizontal,
  X,
} from "lucide-react";
import { useAuth } from "./auth";
import { Modal } from "./Modal";
import { ImageLightbox, type LightboxImage } from "./ImageLightbox";
import { IdentificationReview, IdentitySummary } from "./Identification";
import { ConditionReview, conditionSummary } from "./ConditionReview";
import { imageBlob, saveBlob, saveJSON, type Api } from "./api";
import {
  batchLabel,
  groupCardCaptures,
  presentCardPair,
  captureLabel,
  dateLabel,
  isTerminal,
  uuid,
  type Batch,
  type EditedImage,
  type JobPage,
  type OriginalImage,
} from "./models";
import { CancelJob, JobRow } from "./Dashboard";
import { Empty, ErrorNote, Field, Loading, StateBadge } from "./ui";

function imagePath(
  id: string,
  sequence: number,
  thumbnail = false,
  editID?: string,
) {
  const params = new URLSearchParams();
  if (thumbnail) params.set("format", "jpeg");
  if (editID) params.set("editID", editID);
  return `images/${id}:${sequence}${thumbnail ? "/thumbnail" : ""}${params.size ? `?${params}` : ""}`;
}
export function Thumbnail({
  id,
  image,
  editID,
}: {
  id: string;
  image: OriginalImage;
  editID?: string;
}) {
  const { api } = useAuth();
  const [url, setURL] = useState("");
  const [error, setError] = useState<unknown>();
  useEffect(() => {
    const controller = new AbortController();
    let objectURL = "";
    setURL("");
    setError(undefined);
    void imageBlob(
      api,
      imagePath(id, image.sequence, true, editID),
      controller.signal,
      image.sha256,
    )
      .then((blob) => {
        if (!controller.signal.aborted) {
          objectURL = URL.createObjectURL(blob);
          setURL(objectURL);
        }
      })
      .catch((error) => {
        if (!controller.signal.aborted) setError(error);
      });
    return () => {
      controller.abort();
      if (objectURL) URL.revokeObjectURL(objectURL);
    };
  }, [api, id, image.sequence, image.sha256, editID]);
  return (
    <div className="thumbnail">
      {url ? (
        <img
          src={url}
          alt={`${captureLabel(image.sequence)}, ${image.side === "unknown" ? "side not assigned" : image.side}`}
        />
      ) : error ? (
        <span className="small">Preview unavailable</span>
      ) : (
        <ImageIcon size={30} className="muted" />
      )}
    </div>
  );
}
export function Library() {
  const { api } = useAuth();
  const batches = useInfiniteQuery({
    queryKey: ["jobs", "library"],
    queryFn: ({ pageParam, signal }) =>
      api.get<JobPage>(`batches?limit=20&offset=${pageParam}`, signal),
    initialPageParam: 0,
    getNextPageParam: (page) => page.nextOffset,
  });
  const jobs = batches.data?.pages.flatMap((page) => page.jobs) || [];
  return (
    <>
      <header className="page-heading">
        <div>
          <p className="eyebrow">EVERY CAPTURE, IN ORDER</p>
          <h1>Batch library</h1>
          <p>Your original images, organised by permanent batch number.</p>
        </div>
        <span className="page-mark">
          <Images size={28} />
        </span>
      </header>
      <section className="panel">
        <div className="section-heading">
          <h2>All batches</h2>
          <span className="muted small">Newest first</span>
        </div>
        <ErrorNote error={batches.error} />
        {batches.isPending ? (
          <Loading />
        ) : !jobs.length ? (
          <Empty title="No batches yet">
            Start a scan from the dashboard to build your library.
          </Empty>
        ) : (
          jobs.map((job) => <JobRow key={job.id} job={job} />)
        )}
        {batches.hasNextPage && (
          <button
            className="button secondary load-more"
            disabled={batches.isFetchingNextPage}
            onClick={() => void batches.fetchNextPage()}
          >
            {batches.isFetchingNextPage ? "Loading…" : "Load more batches"}
          </button>
        )}
      </section>
    </>
  );
}
async function exportBatch(api: Api, batch: Batch) {
  const all = [
    ...batch.images.map((image) => ({
      image,
      editID: undefined as string | undefined,
    })),
    ...(batch.processedImages || []).map((edit) => ({
      image: edit.image,
      editID: edit.request.id,
    })),
  ];
  if (
    all.reduce((total, item) => total + item.image.bytes, 0) >
    64 * 1024 * 1024
  )
    throw new Error(
      "This batch exceeds the 64 MB browser export limit. Download images individually or export from the Mac app.",
    );
  const folder = batchLabel(batch);
  const files: Record<string, Uint8Array> = {};
  const manifest = structuredClone(batch);
  for (const item of all) {
    const blob = await imageBlob(
      api,
      imagePath(batch.id, item.image.sequence, false, item.editID),
    );
    const name = item.editID
      ? `edit-${item.editID.toLowerCase()}.png`
      : `${captureLabel(item.image.sequence)}.${batch.request.format}`;
    files[`${folder}/${name}`] = new Uint8Array(await blob.arrayBuffer());
    if (!item.editID)
      manifest.images.find(
        (image) => image.sequence === item.image.sequence,
      )!.relativePath = name;
  }
  files[`${folder}/manifest.json`] = new TextEncoder().encode(
    JSON.stringify(manifest, null, 2),
  );
  const worker = new Worker(new URL("./zip-worker.ts", import.meta.url), {
    type: "module",
  });
  const blob = await new Promise<Blob>((resolve, reject) => {
    worker.onmessage = (event) => {
      worker.terminate();
      resolve(new Blob([event.data], { type: "application/zip" }));
    };
    worker.onerror = () => {
      worker.terminate();
      reject(new Error("Could not create the export archive."));
    };
    worker.postMessage(
      files,
      Object.values(files).map((data) => data.buffer),
    );
  });
  saveBlob(blob, `${folder}.zip`);
}
export function BatchDetail({ id }: { id: string }) {
  const { api } = useAuth();
  const batch = useQuery({
    queryKey: ["batch", id],
    queryFn: ({ signal }) =>
      api.get<Batch>(`batches/${encodeURIComponent(id)}`, signal),
    refetchInterval: (query) =>
      query.state.data &&
      (!isTerminal(query.state.data.state) ||
        query.state.data.cardIdentifications?.some(
          (value) => value.state === "processing",
        ))
        ? 1500
        : false,
  });
  const [lightbox, setLightbox] = useState<{
    images: LightboxImage[];
    initial: number;
  }>();
  const [comparison, setComparison] = useState<number[]>([]);
  const [compareOpen, setCompareOpen] = useState(false);
  const [editing, setEditing] = useState<OriginalImage>();
  const [conditionPair, setConditionPair] = useState<{
    sides: { sequence: number; label: string }[];
    initial: number;
  }>();
  const [identifying, setIdentifying] = useState<number>();
  const queries = useQueryClient();
  const [preparationIDs, setPreparationIDs] = useState<Record<number, string>>(
    {},
  );
  const prepare = useMutation({
    mutationFn: ({
      sequence,
      requestID,
    }: {
      sequence: number;
      requestID: string;
    }) =>
      api.send(`images/${id}:${sequence}/prepare-card`, "POST", {
        id: requestID,
      }),
    onSuccess: () => {
      void queries.invalidateQueries({ queryKey: ["batch", id] });
    },
  });
  const download = useMutation({
    mutationFn: async ({
      image,
      editID,
    }: {
      image: OriginalImage;
      editID?: string;
    }) => {
      const blob = await imageBlob(
        api,
        imagePath(id, image.sequence, false, editID),
      );
      saveBlob(
        blob,
        `${batch.data ? batchLabel(batch.data) : id}-${editID ? (batch.data?.processedImages?.find((edit) => edit.request.id === editID)?.cardPreparation ? `${captureLabel(image.sequence)}-card.png` : `edit-${editID}.png`) : `${captureLabel(image.sequence)}.${batch.data?.request.format || "png"}`}`,
      );
    },
  });
  const archive = useMutation({
    mutationFn: () => exportBatch(api, batch.data!),
  });
  if (batch.isPending) return <Loading label="Opening batch…" />;
  if (!batch.data) return <ErrorNote error={batch.error} />;
  const record = batch.data;
  const finished = isTerminal(record.state);
  const grouped = record.request.duplex && record.request.prepareCards === true;
  function tile(image: OriginalImage, edit?: EditedImage, side?: string) {
    const prepared = !edit
      ? record.processedImages
          ?.filter(
            (value) =>
              value.request.sequence === image.sequence &&
              value.cardPreparation,
          )
          .at(-1)
      : undefined;
    const identification = record.cardIdentifications?.find(
      (value) => value.request.sequence === image.sequence,
    );
    return (
      <article className="image-card" key={edit?.request.id || image.sequence}>
        <button
          className="preview-button"
          aria-label={`Enlarge ${captureLabel(image.sequence)}`}
          onClick={(event) => {
            event.currentTarget.focus();
            setLightbox({
              images: [
                {
                  ...(prepared?.image || image),
                  editID: prepared?.request.id || edit?.request.id,
                  label: captureLabel(image.sequence),
                },
              ],
              initial: 0,
            });
          }}
        >
          <Thumbnail
            id={id}
            image={prepared?.image || image}
            editID={prepared?.request.id || edit?.request.id}
          />
        </button>
        <div className="image-card-body">
          <div className="section-heading">
            <h3>{side || captureLabel(image.sequence)}</h3>
            <span className="badge">
              {prepared
                ? "Prepared card"
                : edit
                  ? "Edited copy"
                  : image.side === "unknown"
                    ? "Side unassigned"
                    : image.side}
            </span>
          </div>
          <p className="muted small">
            {side ? `Capture ${captureLabel(image.sequence)} · ` : ""}
            {(prepared?.image || image).width} ×{" "}
            {(prepared?.image || image).height} ·{" "}
            {((prepared?.image || image).bytes / 1024 / 1024).toFixed(1)} MB
          </p>
          <div className="image-actions">
            {!edit && (
              <button
                className="text-button"
                disabled={!finished || prepare.isPending || !!prepared}
                onClick={() => {
                  const requestID = preparationIDs[image.sequence] || uuid();
                  setPreparationIDs({
                    ...preparationIDs,
                    [image.sequence]: requestID,
                  });
                  prepare.mutate({ sequence: image.sequence, requestID });
                }}
              >
                {prepare.isPending &&
                prepare.variables.sequence === image.sequence
                  ? "Preparing…"
                  : prepared
                    ? "Card prepared"
                    : "Prepare card"}
              </button>
            )}
            {prepared && (
              <button
                className="text-button"
                disabled={download.isPending || archive.isPending}
                onClick={() =>
                  download.mutate({
                    image: prepared.image,
                    editID: prepared.request.id,
                  })
                }
              >
                Download card
              </button>
            )}
            {!edit && (
              <button
                className="text-button"
                disabled={!finished}
                onClick={() => setIdentifying(image.sequence)}
              >
                {identification?.state === "processing"
                  ? "Identifying…"
                  : identification
                    ? "Review identity"
                    : "Identify card"}
              </button>
            )}
            <button
              className="text-button"
              disabled={download.isPending || archive.isPending}
              onClick={() =>
                download.mutate({ image, editID: edit?.request.id })
              }
            >
              <Download size={15} />
              {prepared ? "Original" : "Download"}
            </button>
            {!edit && (
              <button
                className="text-button"
                disabled={!finished}
                onClick={() => setEditing(image)}
              >
                <SlidersHorizontal size={15} />
                Edit copy
              </button>
            )}
          </div>
          {prepared?.cardPreparation?.warnings.map((warning, index) => (
            <p key={index} className="small muted">
              {warning}
            </p>
          ))}
          {!edit && !side && (
            <IdentitySummary
              identification={identification}
              pending={!finished && record.request.prepareCards === true}
            />
          )}
          {!edit && (
            <label className="compare-check">
              <input
                type="checkbox"
                checked={comparison.includes(image.sequence)}
                disabled={
                  !comparison.includes(image.sequence) && comparison.length >= 2
                }
                onChange={(e) =>
                  setComparison(
                    e.target.checked
                      ? [...comparison, image.sequence]
                      : comparison.filter((seq) => seq !== image.sequence),
                  )
                }
              />
              Select to compare
            </label>
          )}
        </div>
      </article>
    );
  }
  return (
    <>
      <Link href="/library" className="back-link">
        <ArrowLeft size={16} />
        Batch library
      </Link>
      <header className="page-heading">
        <div>
          <p className="eyebrow">{dateLabel(record.createdAt)}</p>
          <h1>{batchLabel(record)}</h1>
          <p>
            {grouped
              ? `${groupCardCaptures(record.images).length} cards · `
              : ""}
            {record.images.length} images · {record.request.dpi} DPI ·{" "}
            {record.request.format.toUpperCase()} · {record.device.name}
          </p>
        </div>
        <StateBadge state={record.state} />
      </header>
      <div className="toolbar">
        <button
          className="button secondary"
          disabled={
            !finished ||
            !record.images.length ||
            archive.isPending ||
            download.isPending
          }
          onClick={() => archive.mutate()}
        >
          <Download size={16} />
          {archive.isPending ? "Preparing archive…" : "Export batch ZIP"}
        </button>
        <button
          className="button secondary"
          onClick={() =>
            saveJSON(record, `${batchLabel(record)}-manifest.json`)
          }
        >
          Download manifest
        </button>
        <button
          className="button secondary"
          disabled={comparison.length !== 2}
          onClick={() => setCompareOpen(true)}
        >
          Compare selected ({comparison.length}/2)
        </button>
        {!finished && <CancelJob id={id} />}
      </div>
      <ErrorNote
        error={batch.error || download.error || archive.error || prepare.error}
      />
      {record.errors.map((error, i) => (
        <ErrorNote key={i} error={error.message} />
      ))}
      <p className="library-note">
        {grouped
          ? "Each card groups two consecutive captures. Front/back labels follow card-recognition evidence; uncertain sides keep numbered labels. Check correspondence before exporting. Originals remain unchanged."
          : "Original captures are kept exactly as delivered. Side labels remain unassigned until front/back pairing is validated."}
      </p>
      {!finished && (
        <Loading
          label={`${record.state} · ${record.images.length} images saved. Total count is unknown until feeding finishes.`}
        />
      )}
      {grouped ? (
        <div className="card-pair-list">
          {groupCardCaptures(record.images).map((pair) => {
            const {
              identity,
              sides,
              warning: sideWarning,
            } = presentCardPair(pair, record.cardIdentifications);
            const views = sides.map((side) => {
              const prepared = record.processedImages
                ?.filter(
                  (edit) =>
                    edit.request.sequence === side.image?.sequence &&
                    edit.cardPreparation,
                )
                .at(-1);
              return {
                ...side,
                prepared,
                display: prepared?.image || side.image,
              };
            });
            const lightboxImages = views
              .filter((view) => view.display)
              .map((view) => ({
                ...view.display!,
                editID: view.prepared?.request.id,
                label: `${captureLabel(pair.number)} · ${view.label}`,
              }));
            const reviewImage =
              sides.find(
                (side) => side.image?.sequence === identity?.request.sequence,
              )?.image || sides[0].image;
            const warnings = [
              ...new Set(
                views.flatMap(
                  (view) => view.prepared?.cardPreparation?.warnings || [],
                ),
              ),
            ];
            return (
              <section
                className="card-pair"
                key={pair.number}
                aria-label={`Card ${captureLabel(pair.number)}`}
              >
                <div className="paired-visuals">
                  {views.map((view, index) => (
                    <div
                      className={`paired-side ${index === 0 ? "main-side" : "back-side"}`}
                      key={view.label}
                    >
                      {view.display ? (
                        <button
                          className="preview-button"
                          aria-label={`Enlarge ${view.label.toLowerCase()} of ${captureLabel(pair.number)}`}
                          onClick={(event) => {
                            event.currentTarget.focus();
                            setLightbox({
                              images: lightboxImages,
                              initial: lightboxImages.findIndex(
                                (image) =>
                                  image.sequence === view.display!.sequence,
                              ),
                            });
                          }}
                        >
                          <Thumbnail
                            id={id}
                            image={view.display}
                            editID={view.prepared?.request.id}
                          />
                        </button>
                      ) : (
                        <p className="small">Image missing</p>
                      )}
                      <span className="side-caption">{view.label}</span>
                    </div>
                  ))}
                </div>
                <div className="paired-info">
                  <div className="section-heading">
                    <h2>{captureLabel(pair.number)}</h2>
                    {(!pair.first || !pair.second) && (
                      <span className="badge">Incomplete pair</span>
                    )}
                  </div>
                  <IdentitySummary
                    identification={identity}
                    pending={!finished && record.request.prepareCards === true}
                  />
                  <p className="small muted">
                    {conditionSummary(
                      record.cardIdentifications?.find(
                        (i) =>
                          i.request.sequence ===
                          sides.find((s) => s.label === "Back")?.image
                            ?.sequence,
                      )?.conditionCheck,
                    )}
                  </p>
                  {sideWarning && <p className="small muted">{sideWarning}</p>}
                  {record.state !== "completed" && (
                    <p className="small muted">
                      {finished
                        ? "Interrupted batch · check card count"
                        : record.state === "processing"
                          ? "Processing cards…"
                          : "Scanning…"}
                    </p>
                  )}
                  <div className="paired-actions">
                    <button
                      className="text-button"
                      disabled={!finished}
                      onClick={() => {
                        const items = sides
                          .filter((s) => s.image)
                          .map((s) => ({
                            sequence: s.image!.sequence,
                            label: s.label,
                          }));
                        setConditionPair({
                          sides: items,
                          initial:
                            items.find((s) => s.label === "Back")?.sequence ||
                            items[0].sequence,
                        });
                      }}
                    >
                      Check condition
                    </button>
                    {reviewImage && (
                      <button
                        className="text-button"
                        disabled={!finished}
                        onClick={() => setIdentifying(reviewImage.sequence)}
                      >
                        {identity ? "Review identity" : "Identify card"}
                      </button>
                    )}
                    {views
                      .filter((view) => view.display)
                      .map((view) => (
                        <button
                          key={view.label}
                          className="text-button"
                          disabled={download.isPending || archive.isPending}
                          onClick={() =>
                            download.mutate({
                              image: view.display!,
                              editID: view.prepared?.request.id,
                            })
                          }
                        >
                          Download {view.label.toLowerCase()}
                        </button>
                      ))}
                  </div>
                  <details className="pair-details">
                    <summary>
                      Edit, originals & checks
                      {warnings.length ? ` (${warnings.length})` : ""}
                    </summary>
                    {views
                      .filter((view) => view.image)
                      .map((view) => (
                        <div className="paired-actions" key={view.label}>
                          <span className="small">{view.label}</span>
                          <button
                            className="text-button"
                            disabled={!finished}
                            onClick={() => setIdentifying(view.image!.sequence)}
                          >
                            Identify / review
                          </button>
                          <button
                            className="text-button"
                            disabled={!finished}
                            onClick={() => setEditing(view.image)}
                          >
                            Edit
                          </button>
                          <button
                            className="text-button"
                            disabled={download.isPending || archive.isPending}
                            onClick={() =>
                              download.mutate({ image: view.image! })
                            }
                          >
                            Original
                          </button>
                          {!view.prepared && (
                            <button
                              className="text-button"
                              disabled={!finished || prepare.isPending}
                              onClick={() => {
                                const requestID =
                                  preparationIDs[view.image!.sequence] ||
                                  uuid();
                                setPreparationIDs({
                                  ...preparationIDs,
                                  [view.image!.sequence]: requestID,
                                });
                                prepare.mutate({
                                  sequence: view.image!.sequence,
                                  requestID,
                                });
                              }}
                            >
                              Prepare card
                            </button>
                          )}
                        </div>
                      ))}
                    {warnings.map((warning) => (
                      <p key={warning} className="small muted">
                        {warning}
                      </p>
                    ))}
                  </details>
                </div>
              </section>
            );
          })}
        </div>
      ) : (
        <div className="image-grid">
          {record.images.map((image) => tile(image))}
        </div>
      )}
      {finished && !record.images.length && (
        <Empty title="No images saved">
          {record.state === "failed"
            ? "Check the job error before trying again."
            : "This batch contains no captured images."}
        </Empty>
      )}
      {!!record.processedImages?.filter((edit) => !edit.cardPreparation)
        .length && (
        <>
          <h2 className="subheading">Manually edited copies</h2>
          <div className="image-grid">
            {record.processedImages
              .filter((edit) => !edit.cardPreparation)
              .map((edit) => tile(edit.image, edit))}
          </div>
        </>
      )}
      <details className="panel details-panel">
        <summary>Batch details & warnings</summary>
        <p className="mono small break-all">{record.outputDirectory}</p>
        {record.warnings.map((message, i) => (
          <p className="small muted" key={i}>
            {message}
          </p>
        ))}
      </details>
      {lightbox && (
        <ImageLightbox
          batchID={id}
          images={lightbox.images}
          initial={lightbox.initial}
          close={() => setLightbox(undefined)}
        />
      )}
      {compareOpen && (
        <Modal close={() => setCompareOpen(false)}>
          <section
            className="modal wide"
            role="dialog"
            aria-modal="true"
            aria-labelledby="compare-title"
          >
            <div className="section-heading">
              <h2 id="compare-title">Compare captures</h2>
              <button
                autoFocus
                aria-label="Close comparison"
                className="icon-button"
                onClick={() => setCompareOpen(false)}
              >
                <X />
              </button>
            </div>
            <p className="muted small">
              These are selected captures; no front/back relationship is
              inferred.
            </p>
            <div className="comparison-grid">
              {record.images
                .filter((image) => comparison.includes(image.sequence))
                .map((image) => (
                  <div key={image.sequence}>
                    <Thumbnail id={id} image={image} />
                    <h3>{captureLabel(image.sequence)}</h3>
                  </div>
                ))}
            </div>
          </section>
        </Modal>
      )}
      {editing && (
        <ImageEditor
          batchID={id}
          image={editing}
          close={() => setEditing(undefined)}
        />
      )}
      {conditionPair && (
        <ConditionReview
          batch={record}
          sides={conditionPair.sides}
          initial={conditionPair.initial}
          close={() => setConditionPair(undefined)}
        />
      )}
      {identifying !== undefined && (
        <IdentificationReview
          batchID={id}
          sequence={identifying}
          close={() => setIdentifying(undefined)}
        />
      )}
    </>
  );
}
function ImageEditor({
  batchID,
  image,
  close,
}: {
  batchID: string;
  image: OriginalImage;
  close: () => void;
}) {
  const { api } = useAuth();
  const queries = useQueryClient();
  const initial = queries
    .getQueryData<Batch>(["batch", batchID])
    ?.processedImages?.filter(
      (edit) =>
        edit.request.sequence === image.sequence && edit.cardPreparation,
    )
    .at(-1);
  const [rotation, setRotation] = useState(initial?.request.quarterTurns || 0);
  const sourceCrop =
    initial?.cardPreparation?.sourceCrop ?? initial?.request.crop;
  const [crop, setCrop] = useState(
    sourceCrop
      ? {
          x: sourceCrop.x * 100,
          y: sourceCrop.y * 100,
          width: sourceCrop.width * 100,
          height: sourceCrop.height * 100,
        }
      : { x: 0, y: 0, width: 100, height: 100 },
  );
  const [editID] = useState(uuid);
  const valid =
    Object.values(crop).every(Number.isFinite) &&
    crop.x >= 0 &&
    crop.y >= 0 &&
    crop.width > 0 &&
    crop.height > 0 &&
    crop.x + crop.width <= 100 &&
    crop.y + crop.height <= 100;
  const edit = useMutation({
    mutationFn: () =>
      api.send(`images/${batchID}:${image.sequence}/edits`, "POST", {
        id: editID,
        quarterTurns: rotation,
        crop: {
          x: crop.x / 100,
          y: crop.y / 100,
          width: crop.width / 100,
          height: crop.height / 100,
        },
      }),
    onSuccess: () => {
      void queries.invalidateQueries({ queryKey: ["batch", batchID] });
      close();
    },
  });
  return (
    <Modal
      close={() => {
        if (!edit.isPending) close();
      }}
    >
      <section
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="edit-title"
      >
        <div className="section-heading">
          <h2 id="edit-title">Edit a copy of {captureLabel(image.sequence)}</h2>
          <button
            autoFocus
            className="icon-button"
            aria-label="Close editor"
            disabled={edit.isPending}
            onClick={close}
          >
            <X />
          </button>
        </div>
        <p className="muted small">
          Crop coordinates are percentages of the original, measured from the
          top-left. Cropping happens before rotation. Your original remains
          unchanged.
        </p>
        <Thumbnail id={batchID} image={image} />
        <fieldset
          disabled={edit.isPending || edit.isError}
          className="form-stack"
        >
          <div className="form-row">
            {(["x", "y", "width", "height"] as const).map((key) => (
              <Field key={key} label={`${key.toUpperCase()} %`}>
                <input
                  type="number"
                  min={0}
                  max={100}
                  step="0.1"
                  value={crop[key]}
                  onChange={(e) =>
                    setCrop({ ...crop, [key]: Number(e.target.value) })
                  }
                />
              </Field>
            ))}
          </div>
          <Field label="Clockwise rotation">
            <select
              value={rotation}
              onChange={(e) => setRotation(Number(e.target.value))}
            >
              {[0, 1, 2, 3].map((n) => (
                <option key={n} value={n}>
                  {n * 90}°
                </option>
              ))}
            </select>
          </Field>
        </fieldset>
        <ErrorNote error={edit.error} />
        {!valid && (
          <p role="alert" className="error-text">
            Keep the crop entirely inside the original image.
          </p>
        )}
        <button
          className="button primary full"
          disabled={!valid || edit.isPending}
          onClick={() => edit.mutate()}
        >
          {edit.isPending
            ? "Saving…"
            : edit.isError
              ? "Retry same edit"
              : "Save edited PNG copy"}
        </button>
      </section>
    </Modal>
  );
}
