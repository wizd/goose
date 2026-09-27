import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GitHubUpdater } from './githubUpdater';

vi.mock('electron', () => ({ app: { getVersion: () => '1.50.0' } }));
vi.mock('./logger', () => ({
  default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));

const originalPlatform = Object.getOwnPropertyDescriptor(process, 'platform')!;
const originalArch = Object.getOwnPropertyDescriptor(process, 'arch')!;
const originalSystemVersion = Object.getOwnPropertyDescriptor(process, 'getSystemVersion');
const manifestUrl = 'https://goose-update.vcorp.ai/goose/latest.json';
const files = {
  'darwin-arm64': 'https://example.invalid/Goose.zip',
  'darwin-x64': 'https://example.invalid/Goose_intel_mac.zip',
  'win32-x64': 'https://example.invalid/Goose-win32-x64.zip',
  'linux-x64': 'https://example.invalid/Goose-linux-x64.zip',
};
const macOS13Manifest = {
  version: '1.51.0',
  minimumMacOSVersion: '13.0.0',
  files,
};

function mockManifest(body: unknown = macOS13Manifest) {
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string) => {
      if (url !== manifestUrl) {
        throw new Error(`Unexpected request: ${url}`);
      }
      return body instanceof Response ? body : new Response(JSON.stringify(body));
    })
  );
}

beforeEach(() => {
  Object.defineProperty(process, 'platform', { value: 'darwin' });
  Object.defineProperty(process, 'arch', { value: 'arm64' });
  Object.defineProperty(process, 'getSystemVersion', {
    value: vi.fn(() => '12.7.6'),
    configurable: true,
  });
});

afterEach(() => {
  Object.defineProperty(process, 'platform', originalPlatform);
  Object.defineProperty(process, 'arch', originalArch);
  if (originalSystemVersion) {
    Object.defineProperty(process, 'getSystemVersion', originalSystemVersion);
  } else {
    Reflect.deleteProperty(process, 'getSystemVersion');
  }
  vi.unstubAllGlobals();
});

describe('GitHub updater macOS compatibility', () => {
  it.each(['arm64', 'x64'])('does not offer a macOS 13 update on macOS 12 (%s)', async (arch) => {
    Object.defineProperty(process, 'arch', { value: arch });
    mockManifest();
    const result = await new GitHubUpdater().checkForUpdates();
    expect(result).toEqual({ updateAvailable: false, latestVersion: '1.51.0' });
    expect(result.downloadUrl).toBeUndefined();
  });

  it.each([
    ['arm64', '13.0', 'Goose.zip'],
    ['x64', '13.0', 'Goose_intel_mac.zip'],
    ['arm64', '26.0', 'Goose.zip'],
  ])('offers the %s download on macOS %s', async (arch, version, asset) => {
    Object.defineProperty(process, 'arch', { value: arch });
    vi.mocked(process.getSystemVersion).mockReturnValue(version);
    mockManifest();
    expect(await new GitHubUpdater().checkForUpdates()).toMatchObject({
      updateAvailable: true,
      downloadUrl: `https://example.invalid/${asset}`,
    });
  });

  it('still offers a macOS 12-compatible release on macOS 12', async () => {
    mockManifest({ ...macOS13Manifest, minimumMacOSVersion: '12.0.0' });
    expect(await new GitHubUpdater().checkForUpdates()).toMatchObject({ updateAvailable: true });
  });

  it('offers a newer local build of the same upstream version', async () => {
    mockManifest({ ...macOS13Manifest, version: '1.50.0+local.2', minimumMacOSVersion: '12.0.0' });
    expect(await new GitHubUpdater().checkForUpdates()).toMatchObject({
      updateAvailable: true,
      latestVersion: '1.50.0+local.2',
      downloadUrl: 'https://example.invalid/Goose.zip',
    });
  });

  it.each([
    {},
    { version: '1.51.0', minimumMacOSVersion: 'invalid', files },
    { version: '1.51.0', minimumMacOSVersion: '13.0.0' },
  ])('rejects malformed update information: %j', async (manifest) => {
    mockManifest(manifest);
    expect(await new GitHubUpdater().checkForUpdates()).toMatchObject({
      updateAvailable: false,
      error: expect.any(String),
    });
  });

  it('does not offer an update without compatibility metadata', async () => {
    mockManifest({ version: '1.51.0', files });
    expect(await new GitHubUpdater().checkForUpdates()).toMatchObject({
      updateAvailable: false,
      error: expect.stringContaining('compatibility information'),
    });
  });

  it('does not offer an update when the manifest cannot be retrieved', async () => {
    mockManifest(new Response('', { status: 503 }));
    expect(await new GitHubUpdater().checkForUpdates()).toMatchObject({
      updateAvailable: false,
      error: expect.any(String),
    });
  });

  it.each(['win32', 'linux'])('leaves %s updates unchanged', async (platform) => {
    Object.defineProperty(process, 'platform', { value: platform });
    Object.defineProperty(process, 'arch', { value: 'x64' });
    mockManifest({ version: '1.51.0', files });
    expect(await new GitHubUpdater().checkForUpdates()).toMatchObject({
      updateAvailable: true,
      downloadUrl: `https://example.invalid/Goose-${platform}-x64.zip`,
    });
  });
});
