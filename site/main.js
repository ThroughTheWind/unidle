// Landing page behavior: the Halo demo, OS detection and download links.
// Static links already point at GitHub's "latest release" asset URLs (versioned at deploy time);
// when the GitHub API answers, they are replaced with the exact asset URLs of the latest release.
(() => {
  const REPO = "ThroughTheWind/unidle";
  const RELEASES = `https://github.com/${REPO}/releases`;

  // ---- Halo demo ----
  const win = document.getElementById("demo");
  const power = document.getElementById("demo-power");
  const state = document.getElementById("demo-state");
  const timer = document.getElementById("demo-timer");
  let started = 0;
  let tick = 0;
  const fmt = (s) => [Math.floor(s / 3600), Math.floor(s / 60) % 60, s % 60].map((n) => String(n).padStart(2, "0")).join(":");
  const render = () => { timer.textContent = `Keeping awake · ${fmt(Math.floor((Date.now() - started) / 1000))}`; };
  power.addEventListener("click", () => {
    const on = power.getAttribute("aria-checked") !== "true";
    power.setAttribute("aria-checked", String(on));
    win.classList.toggle("is-on", on);
    state.textContent = on ? "On" : "Off";
    clearInterval(tick);
    if (on) { started = Date.now(); render(); tick = setInterval(render, 1000); }
    else timer.textContent = "Stopped";
  });

  // ---- OS detection ----
  const ua = `${navigator.userAgentData?.platform ?? ""} ${navigator.platform ?? ""} ${navigator.userAgent}`.toLowerCase();
  const mobile = /android|iphone|ipad|ipod/.test(ua);
  const os = mobile ? null : /win/.test(ua) ? "windows" : /mac/.test(ua) ? "macos" : /linux|x11|cros/.test(ua) ? "linux" : null;
  const names = { windows: "Windows", macos: "macOS", linux: "Linux" };
  const card = os && document.querySelector(`.dl-card[data-os="${os}"]`);
  const hero = document.getElementById("hero-download");
  const heroLabel = document.getElementById("hero-download-label");
  if (card) {
    card.classList.add("is-recommended");
    heroLabel.textContent = `Download for ${names[os]}`;
  } else {
    // No match (phone, unknown OS): recommend nothing and send the hero button to the cards.
    heroLabel.textContent = "Download for desktop";
  }
  const syncHero = () => { if (card) hero.href = card.querySelector(".dl-main").href; };

  // ---- Download links ----
  const links = [...document.querySelectorAll("a[data-asset]")];
  const status = document.getElementById("release-status");
  // Opened from the source tree, the version placeholder was never replaced: use the releases page.
  if (links.some((a) => a.href.includes("__VERSION__"))) links.forEach((a) => { a.href = RELEASES; });
  syncHero();

  fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers: { Accept: "application/vnd.github+json" } })
    .then((r) => {
      if (r.status === 404) return null; // no published release yet
      if (!r.ok) throw new Error(String(r.status));
      return r.json();
    })
    .then((release) => {
      if (!release) {
        links.forEach((a) => { a.href = RELEASES; });
        status.innerHTML = "";
        const note = document.createElement("span");
        note.className = "notice";
        note.textContent = "The first release is on its way. The buttons open GitHub Releases until it ships.";
        status.append(note);
        syncHero();
        return;
      }
      for (const a of links) {
        const asset = release.assets.find((x) => x.name.endsWith(a.dataset.asset));
        if (asset) a.href = asset.browser_download_url;
      }
      document.querySelectorAll(".dl-main").forEach((a) => {
        const v = document.createElement("span");
        v.className = "dl-version";
        v.textContent = `${release.tag_name} · released ${new Date(release.published_at).toLocaleDateString()}`;
        a.insertAdjacentElement("afterend", v);
      });
      syncHero();
    })
    .catch(() => { /* rate-limited or offline: keep the static latest-release links */ });
})();
