import { pathToFileURL } from "node:url";

const localBuildPattern = /\+local\.(\d+)$/;

export function localBuildNumber(version) {
  const normalized = normalizeReleaseVersion(version);
  const match = normalized.match(localBuildPattern);
  return match ? Number(match[1]) : 0;
}

export function normalizeReleaseVersion(version) {
  if (version.startsWith("desktop-")) {
    return version.slice("desktop-".length);
  }
  if (/^v\d/.test(version)) {
    return version.slice(1);
  }
  return version;
}

export function upstreamVersion(version) {
  return normalizeReleaseVersion(version).replace(localBuildPattern, "");
}

export function nextDesktopVersion(upstream, versions, desktopVersion = "") {
  const base = upstreamVersion(upstream);
  const publishedMax = versions.reduce(
    (max, version) => Math.max(max, localBuildNumber(version)),
    0,
  );
  const prepared = normalizeReleaseVersion(desktopVersion);
  const preparedNumber = localBuildNumber(prepared);
  if (upstreamVersion(prepared) === base && preparedNumber === publishedMax + 1) {
    return prepared;
  }
  const maxBuild = Math.max(publishedMax, preparedNumber);
  return `${base}+local.${maxBuild + 1}`;
}

function readArg(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) {
    throw new Error(`Missing ${name}`);
  }
  return process.argv[index + 1];
}

const isDirectRun =
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href;

if (isDirectRun) {
  const upstream = readArg("--upstream");
  const desktopIndex = process.argv.indexOf("--desktop");
  const desktop =
    desktopIndex !== -1 && process.argv[desktopIndex + 1]
      ? process.argv[desktopIndex + 1]
      : "";
  const versions = [];
  const tagsIndex = process.argv.indexOf("--tags");
  if (tagsIndex !== -1 && process.argv[tagsIndex + 1]) {
    versions.push(
      ...process.argv[tagsIndex + 1].split(/\s+/).filter((tag) => tag.length > 0),
    );
  }
  process.stdout.write(`${nextDesktopVersion(upstream, versions, desktop)}\n`);
}
