(() => {
  const textarea = document.querySelector("#raw-editor");
  const host = document.querySelector("#visual-editor");
  const form = document.querySelector(".editor-form[data-draft-key]");
  if (!textarea || !host || !form || !window.pell) return;

  const storageKey = `ashes:draft:${form.dataset.draftKey}`;
  const maxAge = 30 * 24 * 60 * 60 * 1000;
  const banner = document.querySelector("#draft-recovery");
  let timer;

  const editor = pell.init({
    element: host,
    defaultParagraphSeparator: "p",
    onChange: (html) => {
      if (textarea.style.display !== "block") textarea.value = html;
      scheduleDraft();
    },
    actions: ["bold", "italic", "underline", "strikethrough", "heading1", "heading2", "paragraph", "quote", "olist", "ulist", "code", "line", "link"]
  });
  editor.content.innerHTML = textarea.value;

  const data = () => {
    if (textarea.style.display !== "block") textarea.value = editor.content.innerHTML;
    return {
      title: form.elements.title.value,
      slug: form.elements.slug.value,
      content_type: form.elements.content_type.value,
      is_gm_secret: form.elements.is_gm_secret.checked,
      body_html: textarea.value,
      edit_summary: form.elements.edit_summary.value
    };
  };
  const baseline = JSON.stringify(data());

  const saveDraft = () => {
    const current = data();
    if (JSON.stringify(current) === baseline) {
      localStorage.removeItem(storageKey);
      return;
    }
    localStorage.setItem(storageKey, JSON.stringify({ savedAt: Date.now(), data: current }));
  };
  function scheduleDraft() {
    clearTimeout(timer);
    timer = setTimeout(saveDraft, 500);
  }

  const applyDraft = (draft) => {
    form.elements.title.value = draft.title;
    form.elements.slug.value = draft.slug;
    form.elements.content_type.value = draft.content_type;
    form.elements.is_gm_secret.checked = Boolean(draft.is_gm_secret);
    form.elements.edit_summary.value = draft.edit_summary;
    textarea.value = draft.body_html;
    editor.content.innerHTML = draft.body_html;
    banner.hidden = true;
  };

  try {
    const stored = JSON.parse(localStorage.getItem(storageKey));
    if (stored && Date.now() - stored.savedAt <= maxAge && JSON.stringify(stored.data) !== baseline) {
      banner.hidden = false;
      banner.querySelector("[data-draft-recover]").addEventListener("click", () => applyDraft(stored.data));
      banner.querySelector("[data-draft-discard]").addEventListener("click", () => {
        localStorage.removeItem(storageKey);
        banner.hidden = true;
      });
    } else if (stored) {
      localStorage.removeItem(storageKey);
    }
  } catch {
    localStorage.removeItem(storageKey);
  }

  document.querySelectorAll("[data-mode]").forEach((button) => button.addEventListener("click", () => {
    const raw = button.dataset.mode === "raw";
    if (raw) {
      textarea.value = editor.content.innerHTML;
      host.style.display = "none";
      textarea.style.display = "block";
    } else {
      editor.content.innerHTML = textarea.value;
      textarea.style.display = "none";
      host.style.display = "block";
    }
    document.querySelectorAll("[data-mode]").forEach((item) => item.classList.toggle("active", item === button));
    scheduleDraft();
  }));

  form.addEventListener("input", scheduleDraft);
  form.addEventListener("change", scheduleDraft);
  form.addEventListener("submit", () => {
    clearTimeout(timer);
    if (textarea.style.display !== "block") textarea.value = editor.content.innerHTML;
    saveDraft();
    sessionStorage.setItem("ashes:clear-draft", storageKey);
  });
})();
