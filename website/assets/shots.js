// Fills the screenshot section from shots.json (written at release from the
// app's real screenshots). The section stays hidden when there are none.
fetch("shots.json", { cache: "no-cache" })
  .then((r) => (r.ok ? r.json() : []))
  .then((shots) => {
    if (!Array.isArray(shots) || shots.length === 0) return;
    const section = document.getElementById("shots");
    const grid = section.querySelector(".shots");
    for (const s of shots) {
      const fig = document.createElement("figure");
      const img = document.createElement("img");
      img.src = s.src;
      img.alt = s.alt;
      img.loading = "lazy";
      img.width = s.width || 1200;
      img.height = s.height || 800;
      const cap = document.createElement("figcaption");
      cap.textContent = s.caption || s.alt;
      fig.append(img, cap);
      grid.append(fig);
    }
    section.hidden = false;
  })
  .catch(() => {});
