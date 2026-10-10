// RepoDNA's service worker, which keeps the web version working without a connection once
// it has been opened: the page, its files, and the demo are kept when it is installed, and
// the analysis program the first time it is used. Analyses are never kept here: they are
// opened from files on the device, and the web interface keeps recent ones itself.
//
// The build fills in VERSION, which changes with every file of the build, and FILES, the
// files to keep at once. Only the web version registers it, never `repodna serve` or the
// desktop app.

const VERSION = "__VERSION__";
const FILES = __FILES__;
const CACHE = `repodna-${VERSION}`;
const scope = self.registration.scope;

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches
      .open(CACHE)
      .then((cache) =>
        cache.addAll(FILES.map((file) => new Request(new URL(file, scope), { cache: "reload" }))),
      )
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter((key) => key.startsWith("repodna-") && key !== CACHE)
            .map((key) => caches.delete(key)),
        ),
      )
      .then(() => self.clients.claim()),
  );
});

/** The network's answer, kept for later when it is a complete one. */
async function fetchAndKeep(request) {
  const response = await fetch(request);
  if (response.ok && response.status === 200 && response.type === "basic") {
    const copy = response.clone();
    void caches.open(CACHE).then((cache) => cache.put(request, copy));
  }
  return response;
}

/** The network's answer, or the kept one when there is no connection. */
async function networkFirst(request, fallback) {
  try {
    return await fetchAndKeep(request);
  } catch (error) {
    const kept = (await caches.match(request)) ?? (fallback && (await caches.match(fallback)));
    if (kept) {
      return kept;
    }
    throw error;
  }
}

self.addEventListener("fetch", (event) => {
  const request = event.request;
  const url = new URL(request.url);
  if (request.method !== "GET" || url.origin !== self.location.origin || !url.href.startsWith(scope)) {
    return;
  }
  // A server's API, should one answer on this address, is never kept.
  if (url.pathname.includes("/api/")) {
    return;
  }
  if (request.mode === "navigate") {
    event.respondWith(networkFirst(request, new URL("./", scope).href));
    return;
  }
  // What tells the page which analysis program there is, checked like the page.
  if (url.pathname.endsWith("/engine/engine.json")) {
    event.respondWith(networkFirst(request));
    return;
  }
  // Everything else is named by its content or by the version, or changes only with a new
  // build, which brings a new cache.
  event.respondWith(caches.match(request).then((kept) => kept ?? fetchAndKeep(request)));
});
