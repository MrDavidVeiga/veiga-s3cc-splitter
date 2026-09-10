import "@fortawesome/fontawesome-free/css/all.min.css";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import noPreviewImage from "./assets/no-preview.png";
import exportIcon from "./assets/icons/export.png";
import zipIcon from "./assets/icons/zip.png";
import clearIcon from "./assets/icons/clear.png";

const translations = {
  en: {
    appTitle: "Veiga's S3CC Splitter",
    langLabel: "Language:",
    pickPackage: "Open package",
    dropHint: "Select or drag a .package or .dbc here",
    pickDestination: "Choose destination folder",
    scanList: "Load list",
    resourcesTitle: "Resources",
    noPackageLoaded: "No package loaded.",
    packageReady: (count) => `${count} found:`,
    selectedCount: (count) => `${count} selected`,
    selectAll: "Select all",
    selectNone: "Select none",
    contentFilter: "Content filter",
    allFilter: "All",
    searchLabel: "Search",
    searchPlaceholder: "Search within the current filter",
    busyDefaultTitle: "Processing",
    busyDefaultMessage: "Please wait.",
    busyReadTitle: "Reading package...",
    busyReadMessage: "Building the resource list from the file.",
    busyExportTitle: "Exporting...",
    busyExportMessage: "Saving resources to folder.",
    busyZipTitle: "Compressing...",
    busyZipMessage: "Creating ZIP from selection.",
    emptyTitle: "Open a package to begin.",
    emptySubtitle: "The list, preview and actions will appear here.",
    previewTitle: "Preview",
    previewEmpty: "Select a resource to see the preview.",
    exportSelection: "Export selection",
    clear: "Clear selection",
    readError: "Failed to read package:",
    splittError: "Export failed:",
  },
  pt: {
    appTitle: "Veiga's S3CC Splitter",
    langLabel: "Idioma:",
    pickPackage: "Abrir package",
    dropHint: "Selecione ou arraste um .package ou .dbc aqui",
    pickDestination: "Escolher pasta de destino",
    scanList: "Carregar lista",
    resourcesTitle: "Recursos",
    noPackageLoaded: "Nenhum package carregado.",
    packageReady: (count) => `${count} ${count === 1 ? "encontrado" : "encontrados"}:`,
    selectedCount: (count) => `${count} selecionados`,
    selectAll: "Selecionar tudo",
    selectNone: "Selecionar nenhum",
    contentFilter: "Filtro de conteÃºdo",
    allFilter: "Todos",
    searchLabel: "Busca",
    searchPlaceholder: "Buscar no filtro atual",
    busyDefaultTitle: "Processando",
    busyDefaultMessage: "Aguarde.",
    busyReadTitle: "Lendo package...",
    busyReadMessage: "Montando a lista de recursos do arquivo.",
    busyExportTitle: "Exportando...",
    busyExportMessage: "Gravando recursos em pasta.",
    busyZipTitle: "Compactando...",
    busyZipMessage: "Criando ZIP com a seleção.",
    emptyTitle: "Abra um package para começar.",
    emptySubtitle: "A lista, preview e ações aparecem aqui.",
    previewTitle: "Preview",
    previewEmpty: "Selecione um recurso para ver o preview.",
    exportSelection: "Exportar seleção",
    clear: "Limpar seleÃ§Ã£o",
    readError: "Falha ao ler package:",
    splittError: "Falha na exportação:",
  },
  es: {
    appTitle: "Veiga's S3CC Splitter",
    langLabel: "Idioma:",
    pickPackage: "Abrir package",
    dropHint: "Selecciona o arrastra un .package o .dbc aquí",
    pickDestination: "Elegir carpeta de destino",
    scanList: "Cargar lista",
    resourcesTitle: "Recursos",
    noPackageLoaded: "Ningún package cargado.",
    packageReady: (count) => `${count} ${count === 1 ? "encontrado" : "encontrados"}:`,
    selectedCount: (count) => `${count} seleccionados`,
    selectAll: "Seleccionar todo",
    selectNone: "Seleccionar ninguno",
    contentFilter: "Filtro de contenido",
    allFilter: "Todos",
    searchLabel: "Buscar",
    searchPlaceholder: "Buscar en el filtro actual",
    busyDefaultTitle: "Procesando",
    busyDefaultMessage: "Espera un momento.",
    busyReadTitle: "Leyendo package...",
    busyReadMessage: "Armando la lista de recursos del archivo.",
    busyExportTitle: "Exportando...",
    busyExportMessage: "Guardando recursos en la carpeta.",
    busyZipTitle: "Comprimiendo...",
    busyZipMessage: "Creando ZIP con la selección.",
    emptyTitle: "Abre un package para empezar.",
    emptySubtitle: "La lista, la vista previa y las acciones aparecerán aquí.",
    previewTitle: "Vista previa",
    previewEmpty: "Selecciona un recurso para ver la vista previa.",
    exportSelection: "Exportar selección",
    clear: "Limpiar selecciÃ³n",
    readError: "Error al leer el package:",
    splittError: "Error en la exportación:",
  },
};

const state = {
  packagePath: "",
  destinationFolder: "",
  resourceType: "all",
  search: "",
  resources: [],
  selectedKeys: [],
  preview: null,
  isBusy: false,
  busyTitle: "",
  busyMessage: "",
  progress: 30,
  language: "en",
  isLangMenuOpen: false,
};

const el = {
  pickPackageBtn: document.querySelector("#pick-package-btn"),
  pickDestinationBtn: document.querySelector("#pick-destination-btn"),
  scanBtn: document.querySelector("#scan-btn"),
  exportBtn: document.querySelector("#export-btn"),
  zipBtn: document.querySelector("#zip-btn"),
  clearBtn: document.querySelector("#clear-btn"),
  selectAllBtn: document.querySelector("#select-all-btn"),
  selectNoneBtn: document.querySelector("#select-none-btn"),
  packageDropZone: document.querySelector("#package-drop-zone"),
  packagePath: document.querySelector("#package-path"),
  destinationPath: document.querySelector("#destination-path"),
  searchInput: document.querySelector("#search-input"),
  searchLabel: document.querySelector("#search-label"),
  contentFilterSelect: document.querySelector("#content-filter-select"),
  contentFilterLabel: document.querySelector("#content-filter-label"),
  typeTabs: document.querySelectorAll(".tab-btn"),
  langDropdown: document.querySelector("#lang-dropdown"),
  langToggleBtn: document.querySelector("#lang-toggle-btn"),
  langCurrentCode: document.querySelector("#lang-current-code"),
  langToggleIcon: document.querySelector("#lang-toggle-icon"),
  langMenu: document.querySelector("#lang-menu"),
  langMenuItems: document.querySelectorAll(".lang-menu-item"),
  langLabel: document.querySelector("#lang-label"),
  appTitle: document.querySelector(".app-title"),
  dropHint: document.querySelector("#package-drop-hint"),
  optThumbs: document.querySelector("#opt-thumbs"),
  optCompression: document.querySelector("#opt-compression"),
  optClone: document.querySelector("#opt-clone"),
  resultsTitle: document.querySelector("#results-title"),
  resultsSummary: document.querySelector("#results-summary"),
  selectedCount: document.querySelector("#selected-count"),
  emptyState: document.querySelector("#empty-state"),
  busyState: document.querySelector("#busy-state"),
  resultsState: document.querySelector("#results-state"),
  busyTitle: document.querySelector("#busy-title"),
  busyMessage: document.querySelector("#busy-message"),
  progressBar: document.querySelector("#progress-bar"),
  resourceList: document.querySelector("#resource-list"),
  previewThumb: document.querySelector("#preview-thumb"),
  previewEmpty: document.querySelector("#preview-empty"),
  previewName: document.querySelector("#preview-name"),
  previewKind: document.querySelector("#preview-kind"),
  previewKey: document.querySelector("#preview-key"),
  previewTitle: document.querySelector(".preview-title"),
  emptyTitle: document.querySelector("#empty-state .state-title"),
  emptySubtitle: document.querySelector("#empty-state .state-subtitle"),
};

function t(key, ...args) {
  const lang = translations[state.language] || translations.en;
  const value = lang[key];
  return typeof value === "function" ? value(...args) : value;
}

function updatePathLabel(node, value) {
  if (!node) return;
  node.textContent = value || "";
  node.classList.toggle("hidden", !value);
}

function normalizeLanguageCode(language) {
  if (!language) return "en";
  const normalized = String(language).trim().toLowerCase().replace("_", "-");
  const base = normalized.split("-")[0];
  if (translations[normalized]) return normalized;
  if (translations[base]) return base;
  return "en";
}

function getSystemLanguage() {
  const candidates = [
    ...(Array.isArray(navigator.languages) ? navigator.languages : []),
    navigator.language,
    navigator.userLanguage,
  ].filter(Boolean);

  for (const candidate of candidates) {
    const resolved = normalizeLanguageCode(candidate);
    if (translations[resolved]) {
      return resolved;
    }
  }

  return "en";
}

function setLanguage(language) {
  state.language = normalizeLanguageCode(language);
  state.isLangMenuOpen = false;
  applyStaticTranslations();
  render();
}

function applyStaticTranslations() {
  if (el.appTitle) el.appTitle.textContent = t("appTitle");
  if (el.langLabel) el.langLabel.textContent = t("langLabel");
  if (el.pickPackageBtn) el.pickPackageBtn.textContent = t("pickPackage");
  if (el.dropHint) el.dropHint.textContent = t("dropHint");
  if (el.pickDestinationBtn) el.pickDestinationBtn.textContent = t("pickDestination");
  if (el.scanBtn) el.scanBtn.textContent = t("scanList");
  if (el.resultsTitle) el.resultsTitle.textContent = t("resourcesTitle");
  if (el.selectAllBtn) el.selectAllBtn.textContent = t("selectAll");
  if (el.selectNoneBtn) el.selectNoneBtn.textContent = t("selectNone");
  if (el.contentFilterLabel) el.contentFilterLabel.textContent = t("contentFilter");
  if (el.searchLabel) el.searchLabel.textContent = t("searchLabel");
  if (el.searchInput) el.searchInput.placeholder = t("searchPlaceholder");
  if (el.previewTitle) el.previewTitle.textContent = t("previewTitle");
  if (el.previewEmpty) el.previewEmpty.textContent = t("previewEmpty");
  if (el.exportBtn) el.exportBtn.innerHTML = `<img src="${exportIcon}" alt="" class="btn-icon" /><span>${t("exportSelection")}</span>`;
  if (el.zipBtn) el.zipBtn.innerHTML = `<img src="${zipIcon}" alt="" class="btn-icon" /><span>${t("busyZipTitle").replace("...", "")}</span>`;
  if (el.clearBtn) el.clearBtn.innerHTML = `<img src="${clearIcon}" alt="" class="btn-icon" /><span>${t("clear")}</span>`;
  if (el.emptyTitle) el.emptyTitle.textContent = t("emptyTitle");
  if (el.emptySubtitle) el.emptySubtitle.textContent = t("emptySubtitle");

  if (el.langCurrentCode) el.langCurrentCode.textContent = state.language.toUpperCase();
  if (el.langToggleBtn) {
    el.langToggleBtn.title = state.language.toUpperCase();
    el.langToggleBtn.setAttribute("aria-expanded", String(state.isLangMenuOpen));
  }
  if (el.langMenu) el.langMenu.classList.toggle("hidden", !state.isLangMenuOpen);
  if (el.langToggleIcon) {
    el.langToggleIcon.className = `fa-solid ${state.isLangMenuOpen ? "fa-angle-up" : "fa-angle-down"}`;
  }
  el.langMenuItems.forEach((btn) => {
    const isActive = btn.dataset.lang === state.language;
    btn.classList.toggle("hidden", isActive);
    btn.classList.toggle("active", isActive);
    btn.title = btn.dataset.lang.toUpperCase();
  });
}

function setBusy(title, message) {
  state.isBusy = true;
  state.busyTitle = title;
  state.busyMessage = message;
  render();
}

function clearBusy() {
  state.isBusy = false;
  state.busyTitle = "";
  state.busyMessage = "";
  render();
}

const contentTypeOrder = [
  "Hair",
  "Facial Hair",
  "Eyebrows",
  "Body Hair",
  "Clothes",
  "Shoes",
  "Accessories",
  "Makeup",
  "Eyes",
  "Skin Details",
  "Tattoos",
  "Teeth",
  "Face Parts",
  "Pet Parts",
  "Pet Coats",
  "Pet Accessories",
  "Other CASP",
  "Sliders",
  "Skins",
  "Patterns",
  "Comfort",
  "Surfaces",
  "Storage",
  "Appliances",
  "Electronics",
  "Plumbing",
  "Lighting",
  "Decor",
  "Entertainment",
  "Kids",
  "Vehicles",
  "Pets",
  "Show Stage",
  "Resort",
  "Debug",
  "Other Objects",
  "Doors & Arches",
  "Windows",
  "Fences & Gates",
  "Stairs & Railings",
  "Columns",
  "Foundations",
  "Walls",
  "Floors",
  "Ceilings",
  "Roofs",
  "Fireplaces & Chimneys",
  "Plants & Rocks",
  "Terrain Paints",
  "Terrain Tools",
  "Pools & Fountains",
  "Elevators",
  "Rabbit Holes",
  "Building Shells",
  "Landmarks",
  "Blueprints",
  "Resort Objects",
  "Other Build",
];

const contentTypeRank = new Map(contentTypeOrder.map((name, index) => [name, index]));

function getDisplayType(item) {
  return item?.contentType || item?.kind || item?.type || item?.resourceType || "";
}

function getDisplayDetails(item) {
  const details = item?.details || item?.detail || "";
  const kind = item?.kind || "";
  if (!kind || details.includes(kind)) return details;
  return details ? `${kind} - ${details}` : kind;
}

function itemContentTypes(item) {
  const values = Array.isArray(item?.contentTypes) ? item.contentTypes : [];
  const normalized = values.filter(Boolean);
  if (normalized.length) return normalized;
  if (item?.contentType) return [item.contentType];
  return item?.kind ? [item.kind] : [];
}

function countContentTypes() {
  const counts = new Map();
  for (const item of state.resources) {
    for (const contentType of itemContentTypes(item)) {
      counts.set(contentType, (counts.get(contentType) || 0) + 1);
    }
  }
  return counts;
}

function sortedContentTypes(counts) {
  return [...counts.keys()].sort((a, b) => {
    const rankA = contentTypeRank.has(a) ? contentTypeRank.get(a) : Number.MAX_SAFE_INTEGER;
    const rankB = contentTypeRank.has(b) ? contentTypeRank.get(b) : Number.MAX_SAFE_INTEGER;
    if (rankA !== rankB) return rankA - rankB;
    return a.localeCompare(b);
  });
}

function renderContentFilter() {
  if (!el.contentFilterSelect) return;
  const counts = countContentTypes();
  const available = sortedContentTypes(counts);
  if (state.resourceType !== "all" && !counts.has(state.resourceType)) {
    state.resourceType = "all";
  }

  const options = [
    { value: "all", label: `${t("allFilter")} (${state.resources.length})` },
    ...available.map((contentType) => ({
      value: contentType,
      label: `${contentType} (${counts.get(contentType)})`,
    })),
  ];

  el.contentFilterSelect.innerHTML = "";
  for (const optionData of options) {
    const option = document.createElement("option");
    option.value = optionData.value;
    option.textContent = optionData.label;
    option.selected = optionData.value === state.resourceType;
    el.contentFilterSelect.appendChild(option);
  }
  el.contentFilterSelect.value = state.resourceType;
}

function itemMatchesContentFilter(item) {
  return state.resourceType === "all" || itemContentTypes(item).includes(state.resourceType);
}

function itemMatchesSearch(item) {
  const query = state.search.trim().toLowerCase();
  if (!query) return true;
  const values = [
    item?.label,
    item?.key,
    item?.kind,
    item?.details,
    item?.contentType,
    ...(Array.isArray(item?.contentTypes) ? item.contentTypes : []),
  ];
  return values.some((value) => String(value || "").toLowerCase().includes(query));
}

function visibleResources() {
  return state.resources.filter((item) => itemMatchesContentFilter(item) && itemMatchesSearch(item));
}

function isPreviewVisible(visibleItems) {
  if (!state.preview?.key) return true;
  return visibleItems.some((item) => item.key === state.preview.key);
}

function clearPreviewIfHidden(visibleItems) {
  if (!isPreviewVisible(visibleItems)) {
    state.preview = null;
  }
}

function render() {
  updatePathLabel(el.packagePath, state.packagePath);
  updatePathLabel(el.destinationPath, state.destinationFolder);

  const visibleItems = visibleResources();
  clearPreviewIfHidden(visibleItems);
  renderContentFilter();

  el.typeTabs.forEach((btn) => btn.classList.toggle("active", btn.dataset.type === state.resourceType));
  el.selectedCount.textContent = t("selectedCount", state.selectedKeys.length);
  el.resultsSummary.textContent = state.packagePath
    ? t("packageReady", visibleItems.length, state.resourceType)
    : t("noPackageLoaded");

  el.busyTitle.textContent = state.busyTitle || t("busyDefaultTitle");
  el.busyMessage.textContent = state.busyMessage || t("busyDefaultMessage");
  el.progressBar.style.width = `${state.progress}%`;

  el.emptyState.classList.toggle("hidden", state.packagePath || state.isBusy || state.resources.length > 0);
  el.busyState.classList.toggle("hidden", !state.isBusy);
  el.resultsState.classList.toggle("hidden", state.isBusy || state.resources.length === 0);

  if (!state.isBusy && state.resources.length === 0 && state.packagePath) {
    el.emptyState.classList.remove("hidden");
  }

  renderList(visibleItems);
  renderPreview();
}

function renderList(items = visibleResources()) {
  el.resourceList.innerHTML = "";

  for (const item of items) {
    const displayType = getDisplayType(item);
    const displayDetails = getDisplayDetails(item);

    const row = document.createElement("div");
    row.className = "resource-row" + (state.preview?.key === item.key ? " active" : "");

    const checkWrap = document.createElement("div");
    checkWrap.className = "resource-check";

    const check = document.createElement("input");
    check.type = "checkbox";
    check.checked = state.selectedKeys.includes(item.key);
    check.addEventListener("change", () => toggleSelection(item.key));
    checkWrap.appendChild(check);

    const button = document.createElement("button");
    button.className = "resource-button";

    const metaLines = [];
    if (displayType) metaLines.push(displayType);
    if (displayDetails) metaLines.push(displayDetails);

    button.innerHTML = `
      <div class="resource-name">${item.label}</div>
      <div class="resource-meta">${metaLines.join("<br />")}</div>
    `;
    button.addEventListener("click", () => loadPreview(item));

    const pill = document.createElement("div");
    pill.className = "type-pill";
    pill.textContent = displayType || "-";

    row.append(checkWrap, button, pill);
    el.resourceList.appendChild(row);
  }
}

function renderPreview() {
  if (!state.preview) {
    el.previewThumb.src = noPreviewImage;
    el.previewThumb.classList.remove("hidden");
    el.previewEmpty.classList.add("hidden");
    el.previewName.textContent = "";
    el.previewKind.textContent = "";
    el.previewKey.textContent = "";
    return;
  }

  if (state.preview.thumbBase64) {
    el.previewThumb.src = `data:image/png;base64,${state.preview.thumbBase64}`;
    el.previewThumb.classList.remove("hidden");
    el.previewEmpty.classList.add("hidden");
  } else {
    el.previewThumb.src = noPreviewImage;
    el.previewThumb.classList.remove("hidden");
    el.previewEmpty.classList.add("hidden");
  }

  const displayType = getDisplayType(state.preview);
  const displayDetails = getDisplayDetails(state.preview);

  el.previewName.textContent = state.preview.label;
  el.previewKind.textContent = displayDetails
    ? `${displayType} • ${displayDetails}`
    : displayType;
  el.previewKey.textContent = "";
}

function toggleSelection(key) {
  if (state.selectedKeys.includes(key)) {
    state.selectedKeys = state.selectedKeys.filter((item) => item !== key);
  } else {
    state.selectedKeys = [...state.selectedKeys, key];
  }
  render();
}

async function pickPackage() {
  const result = await open({ multiple: false, filters: [{ name: "Package", extensions: ["package", "dbc"] }] });
  if (typeof result === "string") {
    state.packagePath = result;
    state.resources = [];
    state.selectedKeys = [];
    state.preview = null;
    state.resourceType = "all";
    state.search = "";
    if (el.searchInput) el.searchInput.value = "";
    render();
  }
}

async function pickDestination() {
  const result = await open({ directory: true, multiple: false });
  if (typeof result === "string") {
    state.destinationFolder = result;
    render();
  }
}

async function scanPackage() {
  if (!state.packagePath) return;

  setBusy(t("busyReadTitle"), t("busyReadMessage"));

  try {
    const result = await invoke("scan_package_resources", {
      payload: {
        packagePath: state.packagePath,
        resourceType: "all",
        search: null,
      },
    });

    state.resources = result.items || [];
    state.selectedKeys = state.resources.map((item) => item.key);
    state.preview = null;
    state.resourceType = "all";

    const firstVisible = visibleResources()[0];
    if (firstVisible) {
      await loadPreview(firstVisible);
    }
  } catch (error) {
    console.error(error);
    alert(`${t("readError")}\n${error}`);
  } finally {
    clearBusy();
  }
}

async function loadPreview(item) {
  try {
    const preview = await invoke("get_resource_preview", {
      payload: {
        packagePath: state.packagePath,
        resourceKey: item.key,
      },
    });
    preview.label = item.label;
    preview.kind = item.kind;
    preview.details = item.details;
    preview.contentType = item.contentType;
    preview.contentTypes = item.contentTypes;
    state.preview = preview;
    render();
  } catch (error) {
    console.error(error);
  }
}

async function exportSelection(zip) {
  if (!state.packagePath || !state.selectedKeys.length || !state.destinationFolder) return;

  const payload = {
    packagePath: state.packagePath,
    destinationFolder: state.destinationFolder,
    selectedKeys: state.selectedKeys,
    includeThumbnails: el.optThumbs.checked,
    enableCompression: el.optCompression.checked,
    createClonePackage: el.optClone.checked,
  };

  setBusy(
    zip ? t("busyZipTitle") : t("busyExportTitle"),
    zip ? t("busyZipMessage") : t("busyExportMessage")
  );

  try {
    const outputPath = await invoke(
      zip ? "zip_selected_resources" : "export_selected_resources",
      { payload }
    );
    await revealItemInDir(outputPath);
  } catch (error) {
    console.error(error);
    alert(`${t("splittError")}\n${error}`);
  } finally {
    clearBusy();
  }
}

function clearSelectionForCurrentFilter() {
  if (state.resourceType === "all") {
    state.selectedKeys = [];
    render();
    return;
  }

  const visibleKeys = new Set(visibleResources().map((item) => item.key));
  state.selectedKeys = state.selectedKeys.filter((key) => !visibleKeys.has(key));
  render();
}

function bindDragAndDrop() {
  const zone = el.packageDropZone;

  ["dragenter", "dragover"].forEach((eventName) => {
    zone.addEventListener(eventName, (event) => {
      event.preventDefault();
      zone.classList.add("drag-over");
    });
  });

  zone.addEventListener("dragleave", (event) => {
    event.preventDefault();
    zone.classList.remove("drag-over");
  });

  zone.addEventListener("drop", (event) => {
    event.preventDefault();
    zone.classList.remove("drag-over");
  });

  let appWindow = null;
  try {
    appWindow = getCurrentWindow();
  } catch (error) {
    return;
  }

  appWindow.onDragDropEvent((event) => {
    if (event.payload.type === "over") {
      zone.classList.add("drag-over");
      return;
    }

    if (event.payload.type === "drop") {
      zone.classList.remove("drag-over");
      const path = (event.payload.paths || [])[0];
      const ext = path?.toLowerCase().split(".").pop();
      if (["package", "dbc"].includes(ext)) {
        state.packagePath = path;
        state.resources = [];
        state.selectedKeys = [];
        state.preview = null;
        state.resourceType = "all";
        state.search = "";
        if (el.searchInput) el.searchInput.value = "";
        render();
      }
      return;
    }

    if (event.payload.type === "leave") {
      zone.classList.remove("drag-over");
    }
  });
}

function setupEvents() {
  el.pickPackageBtn.addEventListener("click", pickPackage);
  el.pickDestinationBtn.addEventListener("click", pickDestination);
  el.scanBtn.addEventListener("click", scanPackage);
  el.exportBtn.addEventListener("click", () => exportSelection(false));

  if (el.zipBtn) {
    el.zipBtn.addEventListener("click", () => exportSelection(true));
  }

  el.clearBtn.addEventListener("click", clearSelectionForCurrentFilter);

  el.selectAllBtn.addEventListener("click", () => {
    const merged = new Set(state.selectedKeys);
    for (const item of visibleResources()) {
      merged.add(item.key);
    }
    state.selectedKeys = [...merged];
    render();
  });

  el.selectNoneBtn.addEventListener("click", () => {
    const visibleKeys = new Set(visibleResources().map((item) => item.key));
    state.selectedKeys = state.selectedKeys.filter((key) => !visibleKeys.has(key));
    render();
  });

  if (el.searchInput) {
    el.searchInput.addEventListener("input", (event) => {
      state.search = event.target.value;
      render();
    });
  }

  if (el.contentFilterSelect) {
    el.contentFilterSelect.addEventListener("change", (event) => {
      state.resourceType = event.target.value || "all";
      render();
    });
  }

  el.typeTabs.forEach((btn) =>
    btn.addEventListener("click", () => {
      state.resourceType = btn.dataset.type;
      render();
    })
  );

  if (el.langToggleBtn) {
    el.langToggleBtn.addEventListener("click", (event) => {
      event.stopPropagation();
      state.isLangMenuOpen = !state.isLangMenuOpen;
      applyStaticTranslations();
      render();
    });
  }

  el.langMenuItems.forEach((btn) => {
    btn.addEventListener("click", (event) => {
      event.stopPropagation();
      setLanguage(btn.dataset.lang);
    });
  });

  document.addEventListener("click", (event) => {
    if (!state.isLangMenuOpen) return;
    if (el.langDropdown?.contains(event.target)) return;
    state.isLangMenuOpen = false;
    applyStaticTranslations();
    render();
  });

  bindDragAndDrop();
}

setupEvents();
setLanguage(getSystemLanguage());
