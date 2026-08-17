const statusRegion = document.querySelector(".copy-status");
let statusTimer;

function announce(message) {
  if (!(statusRegion instanceof HTMLElement)) return;
  statusRegion.textContent = message;
  statusRegion.classList.add("visible");
  window.clearTimeout(statusTimer);
  statusTimer = window.setTimeout(() => statusRegion.classList.remove("visible"), 1800);
}

async function copyText(value) {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(value);
      return;
    } catch {
      // Fall through for browsers that expose Clipboard API without granting it.
    }
  }

  const input = document.createElement("textarea");
  input.value = value;
  input.setAttribute("readonly", "");
  input.style.position = "fixed";
  input.style.opacity = "0";
  document.body.append(input);
  input.select();
  const copied = document.execCommand("copy");
  input.remove();
  if (!copied) throw new Error("The browser rejected the copy command");
}

for (const button of document.querySelectorAll("[data-copy-target], [data-copy-value]")) {
  button.addEventListener("click", async () => {
    const targetId = button.getAttribute("data-copy-target");
    const target = targetId ? document.getElementById(targetId) : null;
    const value = button.getAttribute("data-copy-value") ?? target?.textContent?.trim();
    if (!value) return;

    try {
      await copyText(value);
      const previous = button.textContent;
      button.textContent = "Copied";
      announce("Command copied");
      window.setTimeout(() => {
        button.textContent = previous;
      }, 1600);
    } catch {
      announce("Copy failed. Select the command manually.");
    }
  });
}

for (const tab of document.querySelectorAll("[data-install-tab]")) {
  tab.addEventListener("click", () => {
    const selected = tab.getAttribute("data-install-tab");
    if (!selected) return;
    for (const candidate of document.querySelectorAll("[data-install-tab]")) {
      const active = candidate === tab;
      candidate.classList.toggle("active", active);
      candidate.setAttribute("aria-selected", String(active));
    }
    for (const panel of document.querySelectorAll("[data-install-panel]")) {
      panel.toggleAttribute("hidden", panel.getAttribute("data-install-panel") !== selected);
    }
  });
}

for (const year of document.querySelectorAll("[data-current-year]")) {
  year.textContent = String(new Date().getFullYear());
}

const revealNodes = document.querySelectorAll("[data-reveal]");
if ("IntersectionObserver" in window && !window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
  document.documentElement.classList.add("has-reveal");
  const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      entry.target.classList.add("revealed");
      observer.unobserve(entry.target);
    }
  }, { rootMargin: "0px 0px -6%", threshold: 0.08 });
  for (const node of revealNodes) observer.observe(node);
}
