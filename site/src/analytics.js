// Google Analytics, on by default; a visitor who rejects it in the banner
// gets nothing loaded on any later visit. Loaded synchronously from <head> so
// the consent check runs before gtag.js is requested.
(() => {
  const ID = "G-MF93YGG6C4";
  const KEY = "analytics-consent";

  // localStorage throws in some private modes and when site data is blocked;
  // treat that as "no choice stored" rather than breaking the page.
  const read = () => {
    try {
      return localStorage.getItem(KEY);
    } catch {
      return null;
    }
  };
  const store = (value) => {
    try {
      localStorage.setItem(KEY, value);
    } catch {}
  };

  const choice = read();
  if (choice === "denied") return;

  window.dataLayer = window.dataLayer || [];
  function gtag() {
    dataLayer.push(arguments);
  }
  window.gtag = gtag;
  gtag("consent", "default", {
    analytics_storage: "granted",
    ad_storage: "granted",
    ad_user_data: "granted",
    ad_personalization: "granted",
  });
  gtag("js", new Date());
  gtag("config", ID);

  const tag = document.createElement("script");
  tag.async = true;
  tag.src = `https://www.googletagmanager.com/gtag/js?id=${ID}`;
  document.head.appendChild(tag);

  if (choice === "granted") return;

  // gtag.js sets _ga on the widest domain it can, so the parent domain is
  // cleared as well as the current host.
  const deleteCookies = () => {
    const expired = "=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/";
    for (const cookie of document.cookie.split(";")) {
      const name = cookie.split("=")[0].trim();
      if (name !== "_ga" && !name.startsWith("_ga_")) continue;
      document.cookie = name + expired;
      document.cookie = name + expired + "; domain=" + location.hostname;
      document.cookie = name + expired + "; domain=.danimar.dev";
    }
  };

  const showBanner = () => {
    const banner = document.createElement("div");
    banner.setAttribute("role", "region");
    banner.setAttribute("aria-label", "Cookie notice");
    banner.className =
      "fixed inset-x-4 bottom-4 z-[60] flex flex-wrap items-center gap-3 rounded-xl border border-neutral-200 bg-white p-4 text-sm text-neutral-700 shadow-lg sm:inset-x-auto sm:right-6 sm:bottom-6 sm:max-w-sm dark:border-neutral-800 dark:bg-neutral-900 dark:text-neutral-300";
    banner.innerHTML = `
      <p class="min-w-0 flex-1 basis-56">This site uses Google Analytics cookies to count visits.</p>
      <div class="flex shrink-0 gap-2">
        <button type="button" data-consent="denied" class="rounded-lg border border-neutral-200 px-3 py-1.5 font-medium text-neutral-800 hover:bg-neutral-50 dark:border-neutral-700 dark:text-white dark:hover:bg-neutral-800">Reject</button>
        <button type="button" data-consent="granted" class="rounded-lg bg-neutral-900 px-3 py-1.5 font-medium text-white hover:bg-neutral-700 dark:bg-white dark:text-neutral-900 dark:hover:bg-neutral-200">OK</button>
      </div>`;
    banner.addEventListener("click", (event) => {
      const value = event.target.closest("[data-consent]")?.dataset.consent;
      if (!value) return;
      store(value);
      if (value === "denied") {
        gtag("consent", "update", {
          analytics_storage: "denied",
          ad_storage: "denied",
          ad_user_data: "denied",
          ad_personalization: "denied",
        });
        window[`ga-disable-${ID}`] = true;
        deleteCookies();
      }
      banner.remove();
    });
    document.body.appendChild(banner);
  };

  if (document.body) showBanner();
  else document.addEventListener("DOMContentLoaded", showBanner);
})();
