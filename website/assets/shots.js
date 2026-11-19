// Fills the screenshot section from shots.json (written at release from the
// apps' real screenshots, with their sizes). The section stays hidden when
// there are none. A screenshot listed without its size takes it from the
// picture once loaded, so it is never drawn at another aspect ratio.
fetch("shots.json", { cache: "no-cache" })
  .then((r) => (r.ok ? r.json() : []))
  .then((shots) => {
    if (!Array.isArray(shots) || shots.length === 0) return;
    const section = document.getElementById("shots");
    const grid = section.querySelector(".shots");
    for (const s of shots) {
      const fig = document.createElement("figure");
      const img = document.createElement("img");
      img.alt = s.alt;
      img.loading = "lazy";
      img.decoding = "async";
      if (s.width > 0 && s.height > 0) {
        img.width = s.width;
        img.height = s.height;
      } else {
        img.addEventListener("load", () => {
          img.width = img.naturalWidth;
          img.height = img.naturalHeight;
        });
      }
      img.src = s.src;
      const cap = document.createElement("figcaption");
      cap.textContent = s.caption || s.alt;
      fig.append(img, cap);
      grid.append(fig);
    }
    section.hidden = false;
  })
  .catch(() => {});
