import { describe, expect, it, vi } from 'vitest';
import { compareDesktopVersions } from './githubUpdater';

vi.mock('electron', () => ({ app: { getVersion: () => '1.52.0+local.1' } }));
vi.mock('./logger', () => ({
  default: { info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));

describe('compareDesktopVersions', () => {
  it('treats a higher local build as newer on the same upstream version', () => {
    expect(compareDesktopVersions('1.52.0+local.6', '1.52.0+local.5')).toBeGreaterThan(0);
    expect(compareDesktopVersions('desktop-1.52.0+local.2', '1.52.0+local.1')).toBeGreaterThan(0);
  });

  it('treats a newer upstream version as newer even when its local build is lower', () => {
    expect(compareDesktopVersions('1.53.0+local.6', '1.52.0+local.5')).toBeGreaterThan(0);
    expect(compareDesktopVersions('1.53.0', '1.52.0+local.9')).toBeGreaterThan(0);
  });

  it('treats a missing local build as zero', () => {
    expect(compareDesktopVersions('1.52.0+local.1', '1.52.0')).toBeGreaterThan(0);
    expect(compareDesktopVersions('v1.52.0', '1.52.0+local.1')).toBeLessThan(0);
  });

  it('treats the same upstream and local build as equal', () => {
    expect(compareDesktopVersions('desktop-1.52.0+local.4', '1.52.0+local.4')).toBe(0);
  });
});
