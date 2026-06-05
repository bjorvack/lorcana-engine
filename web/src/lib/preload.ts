/**
 * Warm the browser cache for a set of image URLs so card art is ready before
 * the board is shown (otherwise lazily-loaded art pops in blank).
 *
 * Resolves once every image has either loaded or failed — failures are ignored
 * so one missing URL can't block the board. `onProgress` fires after each image
 * settles, for a loading indicator.
 */
export function preloadImages(
  urls: readonly (string | undefined)[],
  onProgress?: (done: number, total: number) => void,
): Promise<void> {
  const unique = [...new Set(urls.filter((u): u is string => !!u))];
  const total = unique.length;

  if (total === 0) {
    onProgress?.(0, 0);
    return Promise.resolve();
  }

  let done = 0;
  return new Promise((resolve) => {
    const settle = (): void => {
      done += 1;
      onProgress?.(done, total);
      if (done === total) resolve();
    };
    for (const url of unique) {
      const img = new Image();
      img.onload = settle;
      img.onerror = settle;
      img.src = url;
    }
  });
}
