// Guarda a escolha manual de idioma para o redirecionamento automático da página inicial respeitar.
document.querySelectorAll("[data-lang]").forEach((a) =>
  a.addEventListener("click", () => {
    try {
      localStorage.setItem("verso-lang", a.dataset.lang);
    } catch {}
  }),
);
