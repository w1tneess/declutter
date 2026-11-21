import { describe, expect, it } from 'vitest';
import { ago, eta, humanSize, plural, relPath } from './format';

describe('format', () => {
  it('humanSize matches v0.2.2', () => {
    expect(humanSize(512)).toBe('512 B');
    expect(humanSize(1536)).toBe('1.5 KB');
    expect(humanSize(1.4 * 1024 ** 3)).toBe('1.4 GB');
  });

  it('plural, ago and eta read as plain language', () => {
    expect(plural(1, 'file')).toBe('1 file');
    expect(plural(2000, 'file')).toBe('2,000 files');
    expect(ago(30)).toBe('just now');
    expect(ago(7200)).toBe('2 hours ago');
    expect(eta(0.2)).toBe('About 1 second left');
    expect(eta(150)).toBe('About 3 minutes left');
  });

  it('relPath is relative to the chosen folder, case-insensitively', () => {
    expect(relPath(String.raw`C:\Users\A\Downloads\x\y.txt`, 'c:\\users\\a\\downloads\\')).toBe(String.raw`x\y.txt`);
    expect(relPath(String.raw`C:\Users\A\Downloads`, String.raw`C:\Users\A\Downloads`)).toBe('.');
    expect(relPath(String.raw`D:\other.txt`, String.raw`C:\Users\A\Downloads`)).toBe(String.raw`D:\other.txt`);
  });
});
