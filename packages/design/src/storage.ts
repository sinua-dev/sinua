/**
 * Browser storage keys for persisted UI state (theme, open disclosures, and the
 * Studio's presets and code drawer). One prefix for everything Sinua stores.
 */
const PREFIX = "sinua.";

export const storageKey = (suffix: string) => `${PREFIX}${suffix}`;

export function readStored(suffix: string): string | null {
  try {
    return window.localStorage.getItem(storageKey(suffix));
  } catch {
    return null; // private window / storage disabled
  }
}

export function writeStored(suffix: string, value: string): boolean {
  try {
    window.localStorage.setItem(storageKey(suffix), value);
    return true;
  } catch {
    return false;
  }
}
