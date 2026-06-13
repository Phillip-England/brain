(() => {
  const SpeechRecognition = window.SpeechRecognition || window.webkitSpeechRecognition || null;
  const defaultKind = "heading";
  const defaultLevel = 1;
  const exportFilename = "BRAIN.md";

  const state = {
    page: "record",
    projects: [],
    currentProject: localStorage.getItem("brain.currentProject") || "",
    ideas: [],
    filteredIdeas: [],
    selectedIds: new Set(),
    selectMode: false,
    recording: false,
    activeKind: defaultKind,
    activeLevel: defaultLevel,
    activeIndex: -1,
    blocks: [],
    recognition: null,
    micStream: null,
    resettingRecognition: false,
    editorDirty: false,
  };

  const els = {
    pages: {
      record: document.querySelector("#recordPage"),
      ideas: document.querySelector("#ideasPage"),
      projects: document.querySelector("#projectsPage"),
      import: document.querySelector("#importPage"),
      export: document.querySelector("#exportPage"),
      controls: document.querySelector("#controlsPage"),
      settings: document.querySelector("#settingsPage"),
    },
    navButtons: document.querySelectorAll("[data-page]"),
    currentProjectLabel: document.querySelector("#currentProjectLabel"),
    exportProjectLabel: document.querySelector("#exportProjectLabel"),
    copyProject: document.querySelector("#copyProject"),
    exportProject: document.querySelector("#exportProject"),
    importMarkdown: document.querySelector("#importMarkdown"),
    importMarkdownFile: document.querySelector("#importMarkdownFile"),
    copyBrain: document.querySelector("#copyBrain"),
    exportBrain: document.querySelector("#exportBrain"),
    logout: document.querySelector("#logout"),
    recordButton: document.querySelector("#recordButton"),
    status: document.querySelector("#status"),
    activeMode: document.querySelector("#activeMode"),
    markdown: document.querySelector("#markdown"),
    toggleRecording: document.querySelector("#toggleRecording"),
    saveIdea: document.querySelector("#saveIdea"),
    undoBlock: document.querySelector("#undoBlock"),
    modeButtons: document.querySelectorAll("[data-mode-kind]"),
    searchInput: document.querySelector("#searchInput"),
    browseMode: document.querySelector("#browseMode"),
    selectMode: document.querySelector("#selectMode"),
    bulkActions: document.querySelector("#bulkActions"),
    selectedCount: document.querySelector("#selectedCount"),
    selectAllIdeas: document.querySelector("#selectAllIdeas"),
    copySelected: document.querySelector("#copySelected"),
    exportSelected: document.querySelector("#exportSelected"),
    deleteSelected: document.querySelector("#deleteSelected"),
    clearSelection: document.querySelector("#clearSelection"),
    ideasList: document.querySelector("#ideasList"),
    createProject: document.querySelector("#createProject"),
    projectsList: document.querySelector("#projectsList"),
    importTextForm: document.querySelector("#importTextForm"),
    importTextInput: document.querySelector("#importTextInput"),
    pasteImportClipboard: document.querySelector("#pasteImportClipboard"),
    clearImportText: document.querySelector("#clearImportText"),
    settingsForm: document.querySelector("#settingsForm"),
    brainDir: document.querySelector("#brainDir"),
    appHome: document.querySelector("#appHome"),
  };

  async function api(path, options = {}) {
    const res = await fetch(path, {
      ...options,
      headers: { "content-type": "application/json", ...(options.headers || {}) },
    });
    if (res.status === 401) {
      location.href = "/login";
      return null;
    }
    if (!res.ok) {
      const contentType = res.headers.get("content-type") || "";
      if (contentType.includes("application/json")) {
        const payload = await res.json();
        throw new Error(payload.error || res.statusText);
      }
      const message = await res.text();
      throw new Error(message.trim() || res.statusText || "Request failed");
    }
    return res.status === 204 ? null : res.json();
  }

  function setPage(page) {
    state.page = page;
    Object.entries(els.pages).forEach(([name, el]) => {
      el.hidden = name !== page;
    });
    els.navButtons.forEach((button) => {
      button.classList.toggle("active", button.dataset.page === page);
    });
    if (page === "ideas") loadProjectIdeas();
    if (page === "projects") renderProjects();
    if (page === "settings") loadSettings();
    if (page === "import") requestAnimationFrame(() => els.importTextInput.focus());
  }

  function setStatus(message, isError = false) {
    els.status.textContent = message;
    els.status.classList.toggle("error", isError);
  }

  function escapeHtml(value) {
    return String(value).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  }

  function formatDate(value) {
    return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value));
  }

  function setCurrentProject(name) {
    state.currentProject = name;
    if (name) localStorage.setItem("brain.currentProject", name);
    else localStorage.removeItem("brain.currentProject");
    renderProjectLabel();
    syncControls();
  }

  function renderProjectLabel() {
    els.currentProjectLabel.textContent = state.currentProject ? state.currentProject : "No project selected";
    els.currentProjectLabel.classList.toggle("empty", !state.currentProject);
    els.exportProjectLabel.textContent = state.currentProject ? state.currentProject : "No project selected";
    els.exportProjectLabel.classList.toggle("empty", !state.currentProject);
  }

  async function loadProjects() {
    state.projects = await api("/api/projects");
    if (state.currentProject && !state.projects.some((project) => project.name === state.currentProject)) {
      setCurrentProject("");
    }
    if (!state.currentProject && state.projects.length) {
      setCurrentProject(state.projects[0].name);
    }
    renderProjectLabel();
    renderProjects();
    if (state.currentProject) await loadProjectIdeas();
    else setStatus("Create or select a project before recording.", true);
  }

  function renderProjects() {
    els.projectsList.innerHTML = state.projects
      .map((project) => `
        <article class="project-card ${project.name === state.currentProject ? "active" : ""}" data-project="${escapeHtml(project.name)}">
          <button class="project-main" data-select-project="${escapeHtml(project.name)}" type="button">
            <strong>${escapeHtml(project.name)}</strong>
            <span>${project.idea_count} ideas${project.name === state.currentProject ? " · selected" : ""}</span>
          </button>
          <div class="project-actions">
            <button data-rename="${escapeHtml(project.name)}" type="button">Rename</button>
            <button data-delete="${escapeHtml(project.name)}" type="button">Delete</button>
          </div>
        </article>`)
      .join("");
  }

  async function loadProjectIdeas() {
    if (!state.currentProject) {
      state.ideas = [];
      renderIdeas();
      return;
    }
    state.ideas = await api(`/api/projects/${encodeURIComponent(state.currentProject)}/ideas`);
    renderIdeas();
  }

  function renderIdeas() {
    const query = els.searchInput.value.trim().toLowerCase();
    state.filteredIdeas = state.ideas.filter((idea) => {
      if (!query) return true;
      return idea.title.toLowerCase().includes(query) || idea.markdown.toLowerCase().includes(query);
    });

    if (!state.currentProject) {
      els.ideasList.innerHTML = `<div class="empty-state">Select a project on the Projects page.</div>`;
      syncBulkActions();
      return;
    }

    if (!state.filteredIdeas.length) {
      els.ideasList.innerHTML = `<div class="empty-state">No ideas found for ${escapeHtml(state.currentProject)}.</div>`;
      syncBulkActions();
      return;
    }

    els.ideasList.innerHTML = state.filteredIdeas.map(ideaHtml).join("");
    syncBulkActions();
  }

  function ideaHtml(idea) {
    const filename = `${filenameSafe(idea.title)}-${idea.id.slice(0, 8)}.md`;
    return `
      <article class="idea-row ${state.selectedIds.has(idea.id) ? "selected" : ""}" data-id="${escapeHtml(idea.id)}">
        <button class="idea-main" data-open-idea="${escapeHtml(idea.id)}" type="button">
          <strong>${escapeHtml(filename)}</strong>
          <span>${escapeHtml(idea.title)} · ${formatDate(idea.created_at)}</span>
        </button>
        <div class="idea-expanded" hidden>
          <pre>${escapeHtml(idea.markdown)}</pre>
          <div class="inline-actions">
            <button data-copy-idea="${escapeHtml(idea.id)}" type="button">Copy</button>
            <button data-export-idea="${escapeHtml(idea.id)}" type="button">Export</button>
          </div>
        </div>
      </article>`;
  }

  function syncBulkActions() {
    els.browseMode.checked = !state.selectMode;
    els.selectMode.checked = state.selectMode;
    els.bulkActions.hidden = !state.selectMode;
    els.selectedCount.textContent = `${state.selectedIds.size} selected`;
    els.selectAllIdeas.disabled = !state.ideas.length;
    els.copySelected.disabled = state.selectedIds.size === 0;
    els.exportSelected.disabled = state.selectedIds.size === 0;
    els.deleteSelected.disabled = state.selectedIds.size === 0;
  }

  function selectedIdeas() {
    return state.ideas.filter((idea) => state.selectedIds.has(idea.id));
  }

  function findIdea(id) {
    return state.ideas.find((idea) => idea.id === id);
  }

  function syncControls() {
    els.toggleRecording.textContent = state.recording ? "Stop" : "Start";
    els.toggleRecording.classList.toggle("is-recording", state.recording);
    els.undoBlock.disabled = state.blocks.length === 0;
    els.saveIdea.disabled = !state.currentProject;
    els.copyProject.disabled = !state.currentProject;
    els.exportProject.disabled = !state.currentProject;
    els.importMarkdown.disabled = false;
    els.modeButtons.forEach((button) => {
      const active = button.dataset.modeKind === state.activeKind && Number(button.dataset.modeLevel) === state.activeLevel;
      button.classList.toggle("is-active", active);
      button.setAttribute("aria-pressed", String(active));
    });
  }

  function modeLabel(kind, level) {
    return kind === "heading" ? `H${level}` : "Paragraph";
  }

  function normalizeText(text) {
    return text.replace(/\s+/g, " ").trim();
  }

  function syncBlocksFromEditor(force = false) {
    if (!force && !state.editorDirty) return;
    const markdown = els.markdown.value.trim();
    if (!markdown) {
      state.blocks = [];
      state.activeIndex = -1;
      state.editorDirty = false;
      return;
    }

    state.blocks = markdown
      .split(/\n{2,}/)
      .map((part) => part.trim())
      .filter(Boolean)
      .map((part) => {
        const heading = part.match(/^(#{1,6})\s+(.+)$/s);
        if (heading) {
          return {
            kind: "heading",
            level: heading[1].length,
            text: normalizeText(heading[2]),
            interim: "",
          };
        }
        return {
          kind: "paragraph",
          level: 0,
          text: normalizeText(part),
          interim: "",
        };
      });

    state.activeIndex = state.blocks.length - 1;
    const active = state.blocks[state.activeIndex];
    state.activeKind = active?.kind || defaultKind;
    state.activeLevel = active?.level || defaultLevel;
    els.activeMode.textContent = modeLabel(state.activeKind, state.activeLevel);
    state.editorDirty = false;
  }

  function ensureBlock(kind = state.activeKind, level = state.activeLevel) {
    const active = state.blocks[state.activeIndex];
    if (active && active.kind === kind && active.level === level) return active;
    return createBlock(kind, level);
  }

  function createBlock(kind = state.activeKind, level = state.activeLevel) {
    const block = { kind, level, text: "", interim: "" };
    state.blocks.push(block);
    state.activeIndex = state.blocks.length - 1;
    state.activeKind = kind;
    state.activeLevel = level;
    renderMarkdown();
    syncControls();
    return block;
  }

  function setMode(kind, level = 0) {
    state.activeKind = kind;
    state.activeLevel = level;
    createBlock(kind, level);
    els.activeMode.textContent = modeLabel(kind, level);
    setStatus(`${modeLabel(kind, level)} ready.`);
    syncControls();
  }

  function appendFinal(text) {
    syncBlocksFromEditor();
    const block = ensureBlock();
    const cleaned = normalizeText(text);
    if (!cleaned) return;
    block.text = block.text ? `${block.text} ${cleaned}` : cleaned;
    block.interim = "";
    renderMarkdown();
  }

  function commitBlockInterim(block) {
    if (!block?.interim) return;
    block.text = block.text ? `${block.text} ${block.interim}` : block.interim;
    block.interim = "";
  }

  function commitActiveInterim() {
    commitBlockInterim(state.blocks[state.activeIndex]);
  }

  function setInterim(text) {
    syncBlocksFromEditor();
    const block = ensureBlock();
    block.interim = normalizeText(text);
    renderMarkdown();
  }

  function markdownForBlock(block) {
    const text = normalizeText(`${block.text} ${block.interim}`.trim());
    if (!text) return "";
    if (block.kind === "heading") return `${"#".repeat(block.level)} ${text}`;
    return text;
  }

  function buildMarkdown(includeInterim = true) {
    return state.blocks
      .map((block) => markdownForBlock(includeInterim ? block : { ...block, interim: "" }))
      .filter(Boolean)
      .join("\n\n");
  }

  function renderMarkdown() {
    const markdown = buildMarkdown(true);
    els.markdown.value = markdown;
    requestAnimationFrame(() => {
      els.markdown.scrollTop = els.markdown.scrollHeight;
    });
  }

  function createRecognition() {
    if (!SpeechRecognition) return null;
    const recognition = new SpeechRecognition();
    recognition.continuous = true;
    recognition.interimResults = true;
    recognition.lang = navigator.language || "en-US";
    recognition.onstart = () => {
      state.resettingRecognition = false;
      setStatus("Recording.");
    };
    recognition.onresult = (event) => {
      if (state.resettingRecognition) return;
      let interim = "";
      for (let index = event.resultIndex; index < event.results.length; index += 1) {
        const result = event.results[index];
        const transcript = result[0]?.transcript || "";
        if (result.isFinal) appendFinal(transcript);
        else interim += transcript;
      }
      setInterim(interim);
    };
    recognition.onerror = (event) => {
      if (event.error === "no-speech" || event.error === "aborted") return;
      setStatus(`Microphone error: ${event.error}`, true);
    };
    recognition.onend = () => {
      if (!state.recording) return;
      window.setTimeout(() => {
        if (!state.recording) return;
        try { recognition.start(); } catch { window.setTimeout(() => state.recording && recognition.start(), 500); }
      }, 250);
    };
    return recognition;
  }

  function resetRecognitionResults() {
    if (!state.recording || !state.recognition) return;
    state.resettingRecognition = true;
    try { state.recognition.abort(); } catch { state.resettingRecognition = false; }
  }

  async function holdMicrophoneOpen() {
    if (!navigator.mediaDevices?.getUserMedia || state.micStream) return;
    state.micStream = await navigator.mediaDevices.getUserMedia({ audio: true });
  }

  function releaseMicrophone() {
    if (!state.micStream) return;
    state.micStream.getTracks().forEach((track) => track.stop());
    state.micStream = null;
  }

  async function startRecording() {
    if (!state.currentProject) {
      setStatus("Select a project on the Projects page first.", true);
      return;
    }
    if (!SpeechRecognition) {
      setStatus("Speech recognition is not available in this browser.", true);
      return;
    }
    ensureBlock();
    state.recording = true;
    els.recordButton.classList.add("is-recording");
    try {
      await holdMicrophoneOpen();
      state.recognition = state.recognition || createRecognition();
      state.recognition.start();
    } catch (error) {
      state.recording = false;
      releaseMicrophone();
      setStatus(error.message || "Could not start microphone.", true);
    }
    syncControls();
  }

  async function stopRecording(showStatus = true) {
    state.recording = false;
    state.blocks.forEach(commitBlockInterim);
    try { state.recognition?.stop(); } catch {}
    releaseMicrophone();
    els.recordButton.classList.remove("is-recording");
    renderMarkdown();
    syncControls();
    if (showStatus) setStatus(buildMarkdown(false) ? "Markdown ready to save." : "Nothing recorded yet.");
  }

  function undoLastBlock() {
    commitActiveInterim();
    state.blocks.pop();
    state.activeIndex = state.blocks.length - 1;
    const active = state.blocks[state.activeIndex];
    state.activeKind = active?.kind || defaultKind;
    state.activeLevel = active?.level || defaultLevel;
    els.activeMode.textContent = modeLabel(state.activeKind, state.activeLevel);
    renderMarkdown();
    syncControls();
    resetRecognitionResults();
  }

  function resetBoard() {
    state.blocks = [];
    state.activeIndex = -1;
    state.activeKind = defaultKind;
    state.activeLevel = defaultLevel;
    els.activeMode.textContent = "H1";
    renderMarkdown();
    syncControls();
  }

  async function saveIdea() {
    await stopRecording(false);
    syncBlocksFromEditor(true);
    const markdown = els.markdown.value.trim() || buildMarkdown(false);
    if (!markdown) {
      setStatus("Nothing to save yet.", true);
      return;
    }
    try {
      const result = await api(`/api/projects/${encodeURIComponent(state.currentProject)}/import-markdown`, {
        method: "POST",
        body: JSON.stringify({ markdown }),
      });
      const copiedMarkdown = result.ideas.map((idea) => idea.markdown.trim()).join("\n\n");
      const ideaLabel = `${result.imported} idea${result.imported === 1 ? "" : "s"}`;
      try {
        await copyText(copiedMarkdown, ideaLabel, false);
        setStatus(`Saved and copied ${ideaLabel}.`);
      } catch (copyError) {
        setStatus(`Saved ${ideaLabel}, but could not copy them: ${copyError.message}`, true);
      }
      resetBoard();
      await loadProjects();
      if (state.page === "ideas") await loadProjectIdeas();
    } catch (error) {
      setStatus(error.message, true);
    }
  }

  function ideasToMarkdown(ideas) {
    return ideas
      .map((idea) => `<!-- project: ${idea.project} | created_at: ${idea.created_at} | id: ${idea.id} -->\n\n${idea.markdown.trim()}`)
      .join("\n\n---\n\n");
  }

  function filenameSafe(value) {
    const cleaned = String(value)
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
    return cleaned || "brain";
  }

  async function copyText(text, label, showStatus = true) {
    if (!text.trim()) {
      setStatus(`No ${label} to copy.`, true);
      return;
    }
    if (navigator.clipboard?.writeText && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
    } else {
      const textarea = document.createElement("textarea");
      textarea.value = text;
      textarea.setAttribute("readonly", "");
      textarea.style.position = "fixed";
      textarea.style.left = "-100vw";
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand("copy");
      textarea.remove();
    }
    if (showStatus) setStatus(`Copied ${label}.`);
  }

  function downloadMarkdown(text, filename, label) {
    if (!text.trim()) {
      setStatus(`No ${label} to export.`, true);
      return;
    }
    const blob = new Blob([text], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = filename;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
    setStatus(`Exported ${label}.`);
  }

  async function projectIdeas(project) {
    return api(`/api/projects/${encodeURIComponent(project)}/ideas`);
  }

  async function copyCurrentProject() {
    if (!state.currentProject) return setStatus("No project selected.", true);
    const ideas = await projectIdeas(state.currentProject);
    await copyText(ideasToMarkdown(ideas), `${state.currentProject} ideas`);
  }

  async function exportCurrentProject() {
    if (!state.currentProject) return setStatus("No project selected.", true);
    const ideas = await projectIdeas(state.currentProject);
    downloadMarkdown(ideasToMarkdown(ideas), exportFilename, `${state.currentProject} ideas`);
  }

  async function importMarkdownText(markdown) {
    if (!markdown?.trim()) return setStatus("No markdown text to import.", true);
    if (isBrainExport(markdown)) {
      const result = await api("/api/import-brain", {
        method: "POST",
        body: JSON.stringify({ markdown }),
      });
      setStatus(`Imported ${result.imported} idea${result.imported === 1 ? "" : "s"} across ${result.projects} project${result.projects === 1 ? "" : "s"}.`);
      await loadProjects();
      if (state.page === "ideas") await loadProjectIdeas();
      return result;
    }
    if (!state.currentProject) return setStatus("No project selected.", true);
    const result = await api(`/api/projects/${encodeURIComponent(state.currentProject)}/import-markdown`, {
      method: "POST",
      body: JSON.stringify({ markdown }),
    });
    setStatus(`Imported ${result.imported} idea${result.imported === 1 ? "" : "s"} into ${state.currentProject}.`);
    await loadProjects();
    if (state.page === "ideas") await loadProjectIdeas();
    return result;
  }

  function isBrainExport(markdown) {
    return /^\s*<!--\s*project\s*:/m.test(markdown);
  }

  async function importMarkdownFile(file) {
    if (!file) return;
    try {
      await importMarkdownText(await file.text());
    } catch (error) {
      setStatus(error.message, true);
    } finally {
      els.importMarkdownFile.value = "";
    }
  }

  async function submitImportText() {
    try {
      await importMarkdownText(els.importTextInput.value);
      els.importTextInput.value = "";
    } catch (error) {
      setStatus(error.message, true);
    }
  }

  async function pasteClipboardIntoImport() {
    if (!navigator.clipboard?.readText || !window.isSecureContext) {
      setStatus("Clipboard read is not available here. Paste into the text box.", true);
      return;
    }
    try {
      els.importTextInput.value = await navigator.clipboard.readText();
      els.importTextInput.focus();
      setStatus("Pasted clipboard text.");
    } catch (error) {
      setStatus(error.message, true);
    }
  }

  async function copyBrain() {
    const ideas = await api("/api/search");
    await copyText(ideasToMarkdown(ideas), "all brain ideas");
  }

  async function exportBrain() {
    const ideas = await api("/api/search");
    downloadMarkdown(ideasToMarkdown(ideas), exportFilename, "all brain ideas");
  }

  async function logout() {
    try {
      await api("/api/logout", { method: "POST" });
    } finally {
      location.href = "/login";
    }
  }

  async function copyIdea(id) {
    const idea = findIdea(id);
    if (idea) await copyText(idea.markdown, idea.title);
  }

  function exportIdea(id) {
    const idea = findIdea(id);
    if (!idea) return;
    downloadMarkdown(idea.markdown, exportFilename, idea.title);
  }

  async function copySelected() {
    await copyText(ideasToMarkdown(selectedIdeas()), "selected ideas");
    state.selectedIds.clear();
    renderIdeas();
  }

  function selectAllProjectIdeas() {
    state.ideas.forEach((idea) => state.selectedIds.add(idea.id));
    renderIdeas();
    setStatus(`Selected all ${state.ideas.length} ideas in ${state.currentProject}.`);
  }

  function exportSelected() {
    downloadMarkdown(ideasToMarkdown(selectedIdeas()), exportFilename, "selected ideas");
  }

  async function deleteSelectedIdeas() {
    const ids = Array.from(state.selectedIds);
    if (!ids.length) return setStatus("No selected ideas to delete.", true);
    const label = `${ids.length} selected idea${ids.length === 1 ? "" : "s"}`;
    if (!confirm(`Delete ${label}? This cannot be undone.`)) return;
    try {
      const result = await api(`/api/projects/${encodeURIComponent(state.currentProject)}/ideas`, {
        method: "DELETE",
        body: JSON.stringify({ ids }),
      });
      state.selectedIds.clear();
      await loadProjects();
      await loadProjectIdeas();
      setStatus(`Deleted ${result.deleted} idea${result.deleted === 1 ? "" : "s"}.`);
    } catch (error) {
      setStatus(error.message, true);
    }
  }

  async function loadSettings() {
    const settings = await api("/api/settings");
    els.brainDir.value = settings.brain_dir;
    els.appHome.textContent = settings.app_home;
  }

  function isEditableTarget(target) {
    return target.closest("input, textarea, select, [contenteditable='true']");
  }

  function handleKeydown(event) {
    if (event.metaKey || event.ctrlKey || event.altKey || state.page !== "record") return;
    const key = event.key.toLowerCase();

    if (key === "u") {
      event.preventDefault();
      syncBlocksFromEditor(true);
      undoLastBlock();
      return;
    }

    if (isEditableTarget(event.target)) return;

    if (event.key === "Enter") {
      event.preventDefault();
      if (!state.recording) startRecording();
      return;
    }

    if (event.key === "\\") {
      event.preventDefault();
      if (state.recording) stopRecording();
      return;
    }

    if (event.key.toLowerCase() === "s") {
      event.preventDefault();
      saveIdea();
      return;
    }

    if (!state.recording) return;
    if (key === "1") {
      event.preventDefault();
      commitActiveInterim();
      setMode("heading", Number(key));
      resetRecognitionResults();
    } else if (key === "p") {
      event.preventDefault();
      commitActiveInterim();
      setMode("paragraph", 0);
      resetRecognitionResults();
    }
  }

  els.navButtons.forEach((button) => button.addEventListener("click", () => setPage(button.dataset.page)));
  els.copyProject.addEventListener("click", copyCurrentProject);
  els.exportProject.addEventListener("click", exportCurrentProject);
  els.importMarkdown.addEventListener("click", () => {
    els.importMarkdownFile.click();
  });
  els.importMarkdownFile.addEventListener("change", () => importMarkdownFile(els.importMarkdownFile.files[0]));
  els.importTextForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    await submitImportText();
  });
  els.pasteImportClipboard.addEventListener("click", pasteClipboardIntoImport);
  els.clearImportText.addEventListener("click", () => {
    els.importTextInput.value = "";
    els.importTextInput.focus();
  });
  els.copyBrain.addEventListener("click", copyBrain);
  els.exportBrain.addEventListener("click", exportBrain);
  els.logout.addEventListener("click", logout);
  els.recordButton.addEventListener("click", () => state.recording ? stopRecording() : startRecording());
  els.toggleRecording.addEventListener("click", () => state.recording ? stopRecording() : startRecording());
  els.saveIdea.addEventListener("click", saveIdea);
  els.undoBlock.addEventListener("click", undoLastBlock);
  els.markdown.addEventListener("input", () => {
    state.editorDirty = true;
    syncBlocksFromEditor();
    syncControls();
  });
  els.modeButtons.forEach((button) => button.addEventListener("click", () => {
    if (state.recording) commitActiveInterim();
    setMode(button.dataset.modeKind, Number(button.dataset.modeLevel));
    resetRecognitionResults();
  }));
  els.searchInput.addEventListener("input", renderIdeas);
  els.browseMode.addEventListener("change", () => {
    if (!els.browseMode.checked) return;
    state.selectMode = false;
    state.selectedIds.clear();
    renderIdeas();
  });
  els.selectMode.addEventListener("change", () => {
    if (!els.selectMode.checked) return;
    state.selectMode = true;
    renderIdeas();
  });
  els.copySelected.addEventListener("click", copySelected);
  els.selectAllIdeas.addEventListener("click", selectAllProjectIdeas);
  els.exportSelected.addEventListener("click", exportSelected);
  els.deleteSelected.addEventListener("click", deleteSelectedIdeas);
  els.clearSelection.addEventListener("click", () => {
    state.selectedIds.clear();
    renderIdeas();
  });
  els.ideasList.addEventListener("click", async (event) => {
    const row = event.target.closest(".idea-row");
    if (state.selectMode && row) {
      const id = row.dataset.id;
      if (state.selectedIds.has(id)) state.selectedIds.delete(id);
      else state.selectedIds.add(id);
      syncBulkActions();
      row.classList.toggle("selected", state.selectedIds.has(id));
      return;
    }

    const copy = event.target.closest("[data-copy-idea]");
    if (copy) {
      await copyIdea(copy.dataset.copyIdea);
      return;
    }

    const exportButton = event.target.closest("[data-export-idea]");
    if (exportButton) {
      exportIdea(exportButton.dataset.exportIdea);
      return;
    }

    const opener = event.target.closest("[data-open-idea]");
    if (!opener) return;
    const openerRow = opener.closest(".idea-row");
    const expanded = openerRow.querySelector(".idea-expanded");
    expanded.hidden = !expanded.hidden;
  });
  els.createProject.addEventListener("submit", async (event) => {
    event.preventDefault();
    const body = Object.fromEntries(new FormData(els.createProject));
    const project = await api("/api/projects", { method: "POST", body: JSON.stringify(body) });
    setCurrentProject(project.name);
    els.createProject.reset();
    await loadProjects();
    setPage("record");
  });
  els.projectsList.addEventListener("click", async (event) => {
    const select = event.target.closest("[data-select-project]");
    const rename = event.target.closest("[data-rename]");
    const remove = event.target.closest("[data-delete]");
    if (select) {
      setCurrentProject(select.dataset.selectProject);
      await loadProjectIdeas();
      renderProjects();
      setStatus(`Project: ${state.currentProject}`);
      return;
    }
    if (rename) {
      const name = prompt("New project name", rename.dataset.rename);
      if (!name) return;
      await api(`/api/projects/${encodeURIComponent(rename.dataset.rename)}`, {
        method: "POST",
        body: JSON.stringify({ name }),
      });
      if (state.currentProject === rename.dataset.rename) setCurrentProject(name);
      await loadProjects();
      return;
    }
    if (remove) {
      const confirmText = prompt("Type exactly: yes I want to delete that");
      if (!confirmText) return;
      await api(`/api/projects/${encodeURIComponent(remove.dataset.delete)}`, {
        method: "DELETE",
        body: JSON.stringify({ confirm: confirmText }),
      });
      if (state.currentProject === remove.dataset.delete) setCurrentProject("");
      await loadProjects();
    }
  });
  els.settingsForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    await api("/api/settings", {
      method: "POST",
      body: JSON.stringify(Object.fromEntries(new FormData(els.settingsForm))),
    });
    await loadSettings();
    await loadProjects();
  });
  document.addEventListener("keydown", handleKeydown);

  els.activeMode.textContent = modeLabel(state.activeKind, state.activeLevel);
  renderProjectLabel();
  syncControls();
  loadProjects().catch((error) => setStatus(error.message, true));
})();
