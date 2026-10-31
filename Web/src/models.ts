import type { Identification } from "./Identification";
export type Format = "png" | "tiff";
export type JobState =
  | "queued"
  | "preparing"
  | "scanning"
  | "processing"
  | "completed"
  | "failed"
  | "cancelled";
export type Failure = { kind: string; message: string };
export type Device = {
  id: string;
  name: string;
  transport?: string;
  driverModuleVersion?: string;
};
export type Settings = { dpi: number; duplex: boolean; format: Format };
export type Profile = Settings & { id: string; name: string };
export type ServiceStatus = {
  activeJobID?: string;
  queuedJobs: number;
  paused: boolean;
  stopping: boolean;
  persistenceError?: Failure;
  storageRoot: string;
};
export type Job = {
  id: string;
  batchNumber?: number;
  scannerID: string;
  state: JobState;
  createdAt: string;
  completedAt?: string;
  progress: { imagesSaved: number };
  outputDirectory: string;
  errors: Failure[];
};
export type OriginalImage = {
  sequence: number;
  relativePath: string;
  width: number;
  height: number;
  bytes: number;
  sha256: string;
  side: string;
  uniformType: string;
};
export type Batch = Omit<Job, "scannerID" | "progress"> & {
  device: Device;
  images: OriginalImage[];
  request: Settings & { prepareCards?: boolean };
  warnings: string[];
  processedImages?: EditedImage[];
  cardIdentifications?: Identification[];
};
export type EditedImage = {
  request: {
    id: string;
    sequence: number;
    crop: { x: number; y: number; width: number; height: number };
    quarterTurns: number;
  };
  image: OriginalImage;
  createdAt: string;
  cardPreparation?: { orientationVerified: boolean; warnings: string[] };
};
export type JobPage = { jobs: Job[]; nextOffset?: number };
export type Setting = {
  current: unknown;
  allowed?: unknown;
  defaultValue?: unknown;
  readOnly: boolean;
  evidence: string;
};
export type Feature = {
  internalName?: string;
  label?: string;
  tooltip?: string;
  fields: Record<string, unknown>;
  evidence: string;
};
export type Inspection = {
  observedAt: string;
  device: Device;
  units: {
    type: number;
    className: string;
    settings: Record<string, Setting>;
    vendorFeatures: Feature[];
    templates: Feature[];
  }[];
  errors: Failure[];
  warnings: string[];
  limitations: string[];
  unknownCapabilities: string[];
};
export type DeviceStatus = {
  device: Device;
  ready?: boolean;
  warnings: string[];
  inspection?: Inspection;
  configuredSettings?: Settings;
};
export type ScanSubmission = Settings & {
  prepareCards?: boolean;
  requestID: string;
  deviceID: string;
  profileID?: string;
};
export const isTerminal = (state: JobState) =>
  ["completed", "failed", "cancelled"].includes(state);
export const batchLabel = (value: { batchNumber?: number; id: string }) =>
  value.batchNumber
    ? `B-${String(value.batchNumber).padStart(3, "0")}`
    : value.id.slice(0, 8);
export const captureLabel = (sequence: number) =>
  `C-${String(sequence).padStart(3, "0")}`;
export const dateLabel = (date: string) =>
  new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(date));
export function controls(inspection?: Inspection) {
  const unit = inspection?.units.find((unit) => unit.type === 3);
  const values = unit?.settings.resolutionDPI?.allowed;
  return {
    resolutions: Array.isArray(values)
      ? values
          .filter(
            (n): n is number =>
              typeof n === "number" &&
              Number.isInteger(n) &&
              n >= 150 &&
              n <= 600,
          )
          .sort((a, b) => a - b)
      : [],
    duplex: unit?.settings.supportsDuplexScanning?.current === true,
  };
}

// getRandomValues also works on the HTTP Tailscale origin; randomUUID requires HTTPS.
export function uuid() {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 15) | 64;
  bytes[8] = (bytes[8] & 63) | 128;
  const hex = [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

// Presentation groups only: never rewrite capture IDs, metadata or physical side labels.
// Sequence slots preserve gaps instead of shifting every subsequent card after a missing image.
export function groupCardCaptures(images: OriginalImage[]) {
  const groups = new Map<
    number,
    { number: number; first?: OriginalImage; second?: OriginalImage }
  >();
  for (const image of images) {
    const number = Math.floor((image.sequence - 1) / 2) + 1;
    const group = groups.get(number) || { number };
    if (image.sequence % 2) group.first = image;
    else group.second = image;
    groups.set(number, group);
  }
  return [...groups.values()].sort((a, b) => a.number - b.number);
}

// Display the side with card identity evidence first, independent of scanner/feed
// direction. This does not rewrite acquisition order or assert verified pairing.
export function presentCardPair(
  pair: ReturnType<typeof groupCardCaptures>[number],
  identifications: Identification[] = [],
) {
  const entries = [pair.first, pair.second].map((image) => ({
    image,
    identity: image
      ? identifications.find(
          (value) => value.request.sequence === image.sequence,
        )
      : undefined,
  }));
  const matched = entries.filter(
    ({ identity }) =>
      identity &&
      (identity.state === "confirmed" || identity.state === "reviewRequired") &&
      (identity.evidence?.candidates.length || 0) > 0,
  );
  if (matched.length === 1 && pair.first && pair.second) {
    const front = matched[0];
    const back = entries.find((entry) => entry !== front)!;
    return {
      identity: front.identity,
      sides: [
        { image: front.image, label: "Front" },
        { image: back.image, label: "Back" },
      ],
      warning: undefined,
    };
  }
  return {
    identity:
      matched[0]?.identity ||
      entries.find(({ identity }) => identity?.state === "processing")
        ?.identity ||
      entries.find(({ identity }) => identity)?.identity,
    sides: entries.map(({ image }, index) => ({
      image,
      label: `Side ${index + 1}`,
    })),
    warning:
      matched.length > 1
        ? "Both sides have card matches; review each side and check the pair."
        : undefined,
  };
}
