import "@fortawesome/fontawesome-free/css/all.min.css";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import noPreviewImage from "./assets/no-preview.png";
import exportIcon from "./assets/icons/export.png";
import zipIcon from "./assets/icons/zip.png";
import clearIcon from "./assets/icons/clear.png";
import {
  availableContentFilters,
  CONTENT_FILTERS,
  contentFilterCounts,
  deselectVisibleItems,
  selectVisibleItems,
  visibleItemsForFilter,
} from "./filter-logic.js";

const translations = {
  en: {
    appTitle: "Veiga's S3CC Splitter",
    langLabel: "Language:",
    pickPackage: "Open package",
    dropHint: "Select or drag a .package or .dbc here",
    pickDestination: "Choose destination folder",
    scanList: "Load list",
    modeSplitter: "Splitter",
    modeMerger: "Merger",
    mergerAddPackages: "Add Packages",
    mergerAddHint: "Select .package/.dbc files or drag files/folders here",
    mergerRun: "Merge Packages",
    mergerClear: "Clear",
    mergerTitle: "Packages to Merge",
    mergerNoPackages: "No packages selected.",
    mergerCount: (count) => `${count} package${count === 1 ? "" : "s"} selected.`,
    mergerBusy: "Merging packages...",
    mergerSuccess: (result) => `Merged successfully: ${result.packageCount} original package(s), ${result.resourceCount} unique resource(s), ${result.sharedResources} shared resource(s).`,
    mergerError: "Merge failed:",
    restoreOriginals: "Restore Original Packages",
    restoreBusyTitle: "Restoring original packages...",
    restoreBusyMessage: "Rebuilding packages from the embedded Merge Manifest.",
    restoreSuccess: (count) => `${count} original package(s) restored successfully.`,
    restoreError: "Restore failed:",
    manifestFound: (count) => `Merge Manifest: ${count} original package(s)`,
    contentFilter: "Filter",
    filterAll: "All",
    filterLabels: {
      "Hair": "Hair",
      "Facial Hair": "Facial Hair",
      "Eyebrows": "Eyebrows",
      "Body Hair": "Body Hair",
      "Clothes": "Clothes",
      "Shoes": "Shoes",
      "Accessories": "Accessories",
      "Makeup": "Makeup",
      "Eyes": "Eyes",
      "Skin Details": "Skin Details",
      "Tattoos": "Tattoos",
      "Teeth": "Teeth",
      "Face Parts": "Face Parts",
      "Pet Parts": "Pet Parts",
      "Pet Coats": "Pet Coats",
      "Pet Accessories": "Pet Accessories",
      "Other CASP": "Other CASP",
      "Sliders": "Sliders",
      "Skins": "Skins",
      "Comfort": "Comfort",
      "Surfaces": "Surfaces",
      "Storage": "Storage",
      "Appliances": "Appliances",
      "Electronics": "Electronics",
      "Plumbing": "Plumbing",
      "Lighting": "Lighting",
      "Decor": "Decor",
      "Entertainment": "Entertainment",
      "Kids": "Kids",
      "Vehicles": "Vehicles",
      "Pets": "Pets",
      "Show Stage": "Show Stage",
      "Resort": "Resort",
      "Debug": "Debug",
      "Other Objects": "Other Objects",
      "Doors & Arches": "Doors & Arches",
      "Windows": "Windows",
      "Fences & Gates": "Fences & Gates",
      "Stairs & Railings": "Stairs & Railings",
      "Columns": "Columns",
      "Foundations": "Foundations",
      "Walls": "Walls",
      "Floors": "Floors",
      "Ceilings": "Ceilings",
      "Roofs": "Roofs",
      "Fireplaces & Chimneys": "Fireplaces & Chimneys",
      "Plants & Rocks": "Plants & Rocks",
      "Terrain Paints": "Terrain Paints",
      "Terrain Tools": "Terrain Tools",
      "Pools & Fountains": "Pools & Fountains",
      "Elevators": "Elevators",
      "Rabbit Holes": "Rabbit Holes",
      "Building Shells": "Building Shells",
      "Landmarks": "Landmarks",
      "Blueprints": "Blueprints",
      "Resort Objects": "Resort Objects",
      "Other Build": "Other Build",
    },
    searchPlaceholder: "Search by name or Instance ID",
    instanceLabel: "Instance",
    duplicateLabel: (count) => `Duplicate ×${count}`,
    resourcesTitle: "Resources",
    noPackageLoaded: "No package loaded.",
    packageReady: (count) => `${count} found:`,
    selectedCount: (count) => `${count} selected`,
    selectAll: "Select all",
    selectNone: "Select none",
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
    removeFromMerged: "Remove from merged",
    removeBusyTitle: "Rebuilding merged package...",
    removeBusyMessage: "Preserving shared resources and validating the new package.",
    removeDestinationRequired: "Choose a destination folder first.",
    removeNothing: "None of the selected resources can be removed safely.",
    removeConfirm: (plan) => `Remove ${plan.resourcesToRemove} resource(s) and ${plan.manifestPackagesToRemove || 0} original package entr${(plan.manifestPackagesToRemove || 0) === 1 ? "y" : "ies"} associated with ${plan.selectedItems} selected item(s)?\n\n${plan.sharedResourcesPreserved} shared resource(s) will be preserved.${plan.selectedResourcesPreserved ? `\n${plan.selectedResourcesPreserved} selected resource(s) are shared and will remain in the merged package.` : ""}\nA bit-for-bit backup of the original will be created before the final replacement.`,
    removeSuccess: (result) => `Merged package rebuilt successfully.\n\nRemoved resources: ${result.removedResources}\nManifest package entries removed: ${result.removedManifestPackages || 0}\nShared resources preserved: ${result.sharedResourcesPreserved}\nBackup: ${result.backupPath}`,
    removeError: "Failed to remove selected items:",
    clear: "Clear",
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
    modeSplitter: "Splitter",
    modeMerger: "Merger",
    mergerAddPackages: "Adicionar Packages",
    mergerAddHint: "Selecione .package/.dbc ou arraste arquivos/pastas aqui",
    mergerRun: "Mesclar Packages",
    mergerClear: "Limpar",
    mergerTitle: "Packages para Mesclar",
    mergerNoPackages: "Nenhum package selecionado.",
    mergerCount: (count) => `${count} package${count === 1 ? "" : "s"} selecionado${count === 1 ? "" : "s"}.`,
    mergerBusy: "Mesclando packages...",
    mergerSuccess: (result) => `Merge concluído: ${result.packageCount} package(s) original(is), ${result.resourceCount} recurso(s) único(s), ${result.sharedResources} recurso(s) compartilhado(s).`,
    mergerError: "Falha ao mesclar:",
    restoreOriginals: "Restaurar Packages Originais",
    restoreBusyTitle: "Restaurando Packages Originais...",
    restoreBusyMessage: "Reconstruindo os packages a partir do Merge Manifest interno.",
    restoreSuccess: (count) => `${count} package(s) original(is) restaurado(s) com sucesso.`,
    restoreError: "Falha ao restaurar:",
    manifestFound: (count) => `Merge Manifest: ${count} package(s) original(is)`,
    contentFilter: "Filtro",
    filterAll: "Todos",
    filterLabels: {
      "Hair": "Cabelo",
      "Facial Hair": "Pelos Faciais",
      "Eyebrows": "Sobrancelhas",
      "Body Hair": "Pelos Corporais",
      "Clothes": "Roupas",
      "Shoes": "Calçados",
      "Accessories": "Acessórios",
      "Makeup": "Maquiagem",
      "Eyes": "Olhos",
      "Skin Details": "Detalhes da Pele",
      "Tattoos": "Tatuagens",
      "Teeth": "Dentes",
      "Face Parts": "Partes do Rosto",
      "Pet Parts": "Partes de Animais",
      "Pet Coats": "Pelagens de Animais",
      "Pet Accessories": "Acessórios de Animais",
      "Other CASP": "Outros CASP",
      "Sliders": "Sliders",
      "Skins": "Peles",
      "Comfort": "Conforto",
      "Surfaces": "Superfícies",
      "Storage": "Armazenamento",
      "Appliances": "Eletrodomésticos",
      "Electronics": "Eletrônicos",
      "Plumbing": "Hidráulica",
      "Lighting": "Iluminação",
      "Decor": "Decoração",
      "Entertainment": "Entretenimento",
      "Kids": "Infantil",
      "Vehicles": "Veículos",
      "Pets": "Animais",
      "Show Stage": "Palco de Show",
      "Resort": "Resort",
      "Debug": "Debug",
      "Other Objects": "Outros Objetos",
      "Doors & Arches": "Portas e Arcos",
      "Windows": "Janelas",
      "Fences & Gates": "Cercas e Portões",
      "Stairs & Railings": "Escadas e Corrimões",
      "Columns": "Colunas",
      "Foundations": "Fundações",
      "Walls": "Paredes",
      "Floors": "Pisos",
      "Ceilings": "Tetos",
      "Roofs": "Telhados",
      "Fireplaces & Chimneys": "Lareiras e Chaminés",
      "Plants & Rocks": "Plantas e Pedras",
      "Terrain Paints": "Pinturas de Terreno",
      "Terrain Tools": "Ferramentas de Terreno",
      "Pools & Fountains": "Piscinas e Fontes",
      "Elevators": "Elevadores",
      "Rabbit Holes": "Rabbit Holes",
      "Building Shells": "Estruturas de Edifícios",
      "Landmarks": "Marcos",
      "Blueprints": "Projetos",
      "Resort Objects": "Objetos de Resort",
      "Other Build": "Outros de Construção",
    },
    searchPlaceholder: "Buscar por nome ou Instance ID",
    instanceLabel: "Instance",
    duplicateLabel: (count) => `Duplicado ×${count}`,
    resourcesTitle: "Recursos",
    noPackageLoaded: "Nenhum package carregado.",
    packageReady: (count) => `${count} ${count === 1 ? "encontrado" : "encontrados"}:`,
    selectedCount: (count) => `${count} selecionados`,
    selectAll: "Selecionar tudo",
    selectNone: "Selecionar nenhum",
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
    removeFromMerged: "Remover do merged",
    removeBusyTitle: "Reconstruindo merged...",
    removeBusyMessage: "Preservando recursos compartilhados e validando o novo package.",
    removeDestinationRequired: "Escolha primeiro uma pasta de destino.",
    removeNothing: "Nenhum dos recursos selecionados pode ser removido com segurança.",
    removeConfirm: (plan) => `Remover ${plan.resourcesToRemove} recurso(s) e ${plan.manifestPackagesToRemove || 0} entrada(s) de package original do manifesto, associados a ${plan.selectedItems} item(ns) selecionado(s)?\n\n${plan.sharedResourcesPreserved} recurso(s) compartilhado(s) serão preservados.${plan.selectedResourcesPreserved ? `\n${plan.selectedResourcesPreserved} recurso(s) selecionado(s) são compartilhados e permanecerão no merged.` : ""}\nUm backup bit a bit do original será criado antes da substituição final.`,
    removeSuccess: (result) => `Merged reconstruído com sucesso.\n\nRecursos removidos: ${result.removedResources}\nEntradas removidas do manifesto: ${result.removedManifestPackages || 0}\nRecursos compartilhados preservados: ${result.sharedResourcesPreserved}\nBackup: ${result.backupPath}`,
    removeError: "Falha ao remover os itens selecionados:",
    clear: "Limpar",
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
    modeSplitter: "Splitter",
    modeMerger: "Merger",
    mergerAddPackages: "Añadir Packages",
    mergerAddHint: "Selecciona .package/.dbc o arrastra archivos/carpetas aquí",
    mergerRun: "Fusionar Packages",
    mergerClear: "Limpiar",
    mergerTitle: "Packages para Fusionar",
    mergerNoPackages: "Ningún package seleccionado.",
    mergerCount: (count) => `${count} package${count === 1 ? "" : "s"} seleccionado${count === 1 ? "" : "s"}.`,
    mergerBusy: "Fusionando packages...",
    mergerSuccess: (result) => `Fusión completada: ${result.packageCount} package(s) original(es), ${result.resourceCount} recurso(s) único(s), ${result.sharedResources} recurso(s) compartido(s).`,
    mergerError: "Error al fusionar:",
    restoreOriginals: "Restaurar Packages Originales",
    restoreBusyTitle: "Restaurando Packages Originales...",
    restoreBusyMessage: "Reconstruyendo los packages desde el Merge Manifest interno.",
    restoreSuccess: (count) => `${count} package(s) original(es) restaurado(s) correctamente.`,
    restoreError: "Error al restaurar:",
    manifestFound: (count) => `Merge Manifest: ${count} package(s) original(es)`,
    contentFilter: "Filtro",
    filterAll: "Todos",
    filterLabels: {
      "Hair": "Cabello",
      "Facial Hair": "Vello Facial",
      "Eyebrows": "Cejas",
      "Body Hair": "Vello Corporal",
      "Clothes": "Ropa",
      "Shoes": "Calzado",
      "Accessories": "Accesorios",
      "Makeup": "Maquillaje",
      "Eyes": "Ojos",
      "Skin Details": "Detalles de la Piel",
      "Tattoos": "Tatuajes",
      "Teeth": "Dientes",
      "Face Parts": "Partes del Rostro",
      "Pet Parts": "Partes de Mascotas",
      "Pet Coats": "Pelajes de Mascotas",
      "Pet Accessories": "Accesorios de Mascotas",
      "Other CASP": "Otros CASP",
      "Sliders": "Sliders",
      "Skins": "Pieles",
      "Comfort": "Comodidad",
      "Surfaces": "Superficies",
      "Storage": "Almacenamiento",
      "Appliances": "Electrodomésticos",
      "Electronics": "Electrónica",
      "Plumbing": "Fontanería",
      "Lighting": "Iluminación",
      "Decor": "Decoración",
      "Entertainment": "Entretenimiento",
      "Kids": "Infantil",
      "Vehicles": "Vehículos",
      "Pets": "Mascotas",
      "Show Stage": "Escenario",
      "Resort": "Resort",
      "Debug": "Debug",
      "Other Objects": "Otros Objetos",
      "Doors & Arches": "Puertas y Arcos",
      "Windows": "Ventanas",
      "Fences & Gates": "Cercas y Portones",
      "Stairs & Railings": "Escaleras y Barandillas",
      "Columns": "Columnas",
      "Foundations": "Cimientos",
      "Walls": "Paredes",
      "Floors": "Suelos",
      "Ceilings": "Techos Interiores",
      "Roofs": "Tejados",
      "Fireplaces & Chimneys": "Chimeneas",
      "Plants & Rocks": "Plantas y Rocas",
      "Terrain Paints": "Pinturas de Terreno",
      "Terrain Tools": "Herramientas de Terreno",
      "Pools & Fountains": "Piscinas y Fuentes",
      "Elevators": "Ascensores",
      "Rabbit Holes": "Madrigueras",
      "Building Shells": "Estructuras de Edificios",
      "Landmarks": "Lugares Emblemáticos",
      "Blueprints": "Planos",
      "Resort Objects": "Objetos de Resort",
      "Other Build": "Otros de Construcción",
    },
    searchPlaceholder: "Buscar por nombre o Instance ID",
    instanceLabel: "Instance",
    duplicateLabel: (count) => `Duplicado ×${count}`,
    resourcesTitle: "Recursos",
    noPackageLoaded: "Ningún package cargado.",
    packageReady: (count) => `${count} ${count === 1 ? "encontrado" : "encontrados"}:`,
    selectedCount: (count) => `${count} seleccionados`,
    selectAll: "Seleccionar todo",
    selectNone: "Seleccionar ninguno",
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
    removeFromMerged: "Eliminar del merged",
    removeBusyTitle: "Reconstruyendo merged...",
    removeBusyMessage: "Conservando recursos compartidos y validando el nuevo package.",
    removeDestinationRequired: "Elige primero una carpeta de destino.",
    removeNothing: "Ninguno de los recursos seleccionados puede eliminarse de forma segura.",
    removeConfirm: (plan) => `¿Eliminar ${plan.resourcesToRemove} recurso(s) y ${plan.manifestPackagesToRemove || 0} entrada(s) de package original del manifiesto, asociados a ${plan.selectedItems} elemento(s) seleccionado(s)?\n\nSe conservarán ${plan.sharedResourcesPreserved} recurso(s) compartido(s).${plan.selectedResourcesPreserved ? `\n${plan.selectedResourcesPreserved} recurso(s) seleccionado(s) son compartidos y permanecerán en el merged.` : ""}\nSe creará una copia de seguridad bit a bit del original antes del reemplazo final.`,
    removeSuccess: (result) => `Merged reconstruido correctamente.\n\nRecursos eliminados: ${result.removedResources}\nEntradas eliminadas del manifiesto: ${result.removedManifestPackages || 0}\nRecursos compartidos conservados: ${result.sharedResourcesPreserved}\nBackup: ${result.backupPath}`,
    removeError: "Error al eliminar los elementos seleccionados:",
    clear: "Limpiar",
    readError: "Error al leer el package:",
    splittError: "Error en la exportación:",
  },
};

const state = {
  mode: "splitter",
  packagePath: "",
  destinationFolder: "",
  search: "",
  resources: [],
  activeContentFilter: "all",
  selectedKeys: [],
  selectionInitialized: false,
  selectionAnchorKey: null,
  preview: null,
  isBusy: false,
  busyTitle: "",
  busyMessage: "",
  progress: 30,
  language: "en",
  isLangMenuOpen: false,
  isContentFilterOpen: false,
  manifestPackageCount: 0,
  mergerSources: [],
  mergerBusy: false,
  mergerResult: null,
};

const el = {
  modeSplitterBtn: document.querySelector("#mode-splitter-btn"),
  modeMergerBtn: document.querySelector("#mode-merger-btn"),
  splitterControls: document.querySelector("#splitter-controls"),
  mergerControls: document.querySelector("#merger-controls"),
  splitterWorkspace: document.querySelector("#splitter-workspace"),
  mergerWorkspace: document.querySelector("#merger-workspace"),
  mergerAddPackagesBtn: document.querySelector("#merger-add-packages-btn"),
  mergerDropZone: document.querySelector("#merger-drop-zone"),
  mergerAddHint: document.querySelector("#merger-add-hint"),
  mergerSourceCount: document.querySelector("#merger-source-count"),
  mergerFooterActions: document.querySelector("#merger-footer-actions"),
  mergerRunBtn: document.querySelector("#merger-run-btn"),
  mergerClearBtn: document.querySelector("#merger-clear-btn"),
  mergerTitle: document.querySelector("#merger-title"),
  mergerSummary: document.querySelector("#merger-summary"),
  mergerSourceList: document.querySelector("#merger-source-list"),
  mergerResult: document.querySelector("#merger-result"),
  restoreManifestBtn: document.querySelector("#restore-manifest-btn"),
  pickPackageBtn: document.querySelector("#pick-package-btn"),
  pickDestinationBtn: document.querySelector("#pick-destination-btn"),
  scanBtn: document.querySelector("#scan-btn"),
  exportBtn: document.querySelector("#export-btn"),
  removeMergedBtn: document.querySelector("#remove-merged-btn"),
  zipBtn: document.querySelector("#zip-btn"),
  clearBtn: document.querySelector("#clear-btn"),
  selectAllBtn: document.querySelector("#select-all-btn"),
  selectNoneBtn: document.querySelector("#select-none-btn"),
  packageDropZone: document.querySelector("#package-drop-zone"),
  packagePath: document.querySelector("#package-path"),
  destinationPath: document.querySelector("#destination-path"),
  searchInput: document.querySelector("#search-input"),
  contentFilter: document.querySelector("#content-filter"),
  contentFilterLabel: document.querySelector("#content-filter-label"),
  contentFilterDropdown: document.querySelector("#content-filter-dropdown"),
  contentFilterToggleBtn: document.querySelector("#content-filter-toggle-btn"),
  contentFilterCurrent: document.querySelector("#content-filter-current"),
  contentFilterToggleIcon: document.querySelector("#content-filter-toggle-icon"),
  contentFilterMenu: document.querySelector("#content-filter-menu"),
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
  if (el.modeSplitterBtn) el.modeSplitterBtn.textContent = t("modeSplitter");
  if (el.modeMergerBtn) el.modeMergerBtn.textContent = t("modeMerger");
  if (el.mergerAddPackagesBtn) el.mergerAddPackagesBtn.textContent = t("mergerAddPackages");
  if (el.mergerAddHint) el.mergerAddHint.textContent = t("mergerAddHint");
  if (el.mergerRunBtn) el.mergerRunBtn.innerHTML = `<img src="${zipIcon}" alt="" class="btn-icon" /><span>${state.mergerBusy ? t("mergerBusy") : t("mergerRun")}</span>`;
  if (el.mergerClearBtn) el.mergerClearBtn.innerHTML = `<img src="${clearIcon}" alt="" class="btn-icon" /><span>${t("mergerClear")}</span>`;
  if (el.mergerTitle) el.mergerTitle.textContent = t("mergerTitle");
  if (el.restoreManifestBtn) el.restoreManifestBtn.textContent = t("restoreOriginals");
  if (el.searchInput) {
    el.searchInput.placeholder = t("searchPlaceholder");
    el.searchInput.setAttribute("aria-label", t("searchPlaceholder"));
  }
  if (el.resultsTitle) el.resultsTitle.textContent = t("resourcesTitle");
  if (el.selectAllBtn) el.selectAllBtn.textContent = t("selectAll");
  if (el.selectNoneBtn) el.selectNoneBtn.textContent = t("selectNone");
  if (el.previewTitle) el.previewTitle.textContent = t("previewTitle");
  if (el.previewEmpty) el.previewEmpty.textContent = t("previewEmpty");
  if (el.exportBtn) el.exportBtn.innerHTML = `<img src="${exportIcon}" alt="" class="btn-icon" /><span>${t("exportSelection")}</span>`;
  if (el.removeMergedBtn) el.removeMergedBtn.textContent = t("removeFromMerged");
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

function getDisplayType(item) {
  return item?.kind || item?.type || item?.resourceType || "";
}

function getDisplayDetails(item) {
  return item?.details || item?.detail || "";
}

function normalizedInstanceQuery(query) {
  const lower = String(query || "").trim().toLowerCase();
  if (!lower) return null;
  const pos = lower.lastIndexOf("0x");
  const candidate = (pos >= 0 ? lower.slice(pos + 2) : lower).trim();
  if (candidate.length < 8 || candidate.length > 16 || !/^[0-9a-f]+$/i.test(candidate)) {
    return null;
  }
  return candidate.toUpperCase().padStart(16, "0");
}

function itemMatchesSearch(item, query) {
  const normalized = String(query || "").trim().toLowerCase();
  if (!normalized) return true;

  const haystack = [
    item?.label,
    item?.key,
    item?.kind,
    item?.details,
    ...(Array.isArray(item?.contentCategories) ? item.contentCategories : []),
  ]
    .filter(Boolean)
    .join("\n")
    .toLowerCase();

  if (haystack.includes(normalized)) return true;

  const instance = normalizedInstanceQuery(normalized);
  if (!instance) return false;

  return String(item?.key || "")
    .split("||")
    .some((part) => {
      const pieces = part.split("-");
      if (pieces.length < 3) return false;
      return pieces[2].replace(/^0x/i, "").toUpperCase().padStart(16, "0") === instance;
    });
}

function getVisibleResources() {
  return visibleItemsForFilter(state.resources, state.activeContentFilter)
    .filter((item) => itemMatchesSearch(item, state.search));
}

function contentFilterLabel(filter) {
  if (filter.value === "all") return t("filterAll");
  const labels = t("filterLabels") || {};
  return labels[filter.value] || filter.label || filter.value;
}

function contentFilterText(filter, counts) {
  return `${contentFilterLabel(filter)} (${counts[filter.value] ?? 0})`;
}

function renderContentFilter() {
  if (!el.contentFilterMenu) return;

  const counts = contentFilterCounts(state.resources);
  const filters = availableContentFilters(state.resources);

  if (!filters.some((filter) => filter.value === state.activeContentFilter)) {
    state.activeContentFilter = "all";
  }

  const activeFilter =
    filters.find((filter) => filter.value === state.activeContentFilter) || CONTENT_FILTERS[0];

  if (el.contentFilterLabel) el.contentFilterLabel.textContent = `${t("contentFilter")}:`;
  if (el.contentFilterCurrent) el.contentFilterCurrent.textContent = contentFilterText(activeFilter, counts);
  if (el.contentFilterToggleBtn) {
    el.contentFilterToggleBtn.setAttribute("aria-expanded", String(state.isContentFilterOpen));
  }
  if (el.contentFilterToggleIcon) {
    el.contentFilterToggleIcon.className =
      `fa-solid ${state.isContentFilterOpen ? "fa-angle-up" : "fa-angle-down"}`;
  }
  el.contentFilterMenu.classList.toggle("hidden", !state.isContentFilterOpen);
  el.contentFilterMenu.setAttribute("aria-label", t("contentFilter"));
  el.contentFilterMenu.innerHTML = "";

  for (const filter of filters) {
    const button = document.createElement("button");
    const isActive = state.activeContentFilter === filter.value;
    button.type = "button";
    button.className = "lang-menu-item content-filter-menu-item" + (isActive ? " active" : "");
    button.textContent = contentFilterText(filter, counts);
    button.dataset.filter = filter.value;
    button.setAttribute("role", "option");
    button.setAttribute("aria-selected", String(isActive));
    button.disabled = isActive;
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      state.activeContentFilter = filter.value;
      state.isContentFilterOpen = false;
      state.selectionAnchorKey = null;
      if (state.preview && !getVisibleResources().some((item) => item.key === state.preview.key)) {
        state.preview = null;
      }
      render();
    });
    el.contentFilterMenu.appendChild(button);
  }
}

function setMode(mode) {
  state.mode = mode === "merger" ? "merger" : "splitter";
  state.isContentFilterOpen = false;
  state.isLangMenuOpen = false;
  applyStaticTranslations();
  render();
}

function fileNameFromPath(path) {
  const normalized = String(path || "").replaceAll("\\", "/");
  return normalized.split("/").pop() || normalized;
}

function renderMergerSources() {
  if (!el.mergerSourceList) return;
  el.mergerSourceList.innerHTML = "";

  if (el.mergerSummary) {
    el.mergerSummary.textContent = state.mergerSources.length
      ? t("mergerCount", state.mergerSources.length)
      : t("mergerNoPackages");
  }
  if (el.mergerSourceCount) {
    el.mergerSourceCount.textContent = state.mergerSources.length
      ? t("mergerCount", state.mergerSources.length)
      : "";
  }

  const hasMergerSources = state.mergerSources.length > 0;
  el.mergerFooterActions?.classList.toggle("hidden", !hasMergerSources);

  state.mergerSources.forEach((path, index) => {
    const row = document.createElement("div");
    row.className = "merger-source-row";

    const info = document.createElement("div");
    const name = document.createElement("div");
    name.className = "merger-source-name";
    name.textContent = fileNameFromPath(path);
    const fullPath = document.createElement("div");
    fullPath.className = "merger-source-path";
    fullPath.textContent = path;
    info.append(name, fullPath);

    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "merger-remove-btn";
    remove.textContent = "×";
    remove.title = t("removeFromMerged");
    remove.addEventListener("click", () => {
      state.mergerSources.splice(index, 1);
      state.mergerResult = null;
      render();
    });

    row.append(info, remove);
    el.mergerSourceList.appendChild(row);
  });

  if (el.mergerRunBtn) {
    el.mergerRunBtn.disabled = state.mergerBusy || state.mergerSources.length < 2;
  }

  if (el.mergerResult) {
    el.mergerResult.classList.toggle("hidden", !state.mergerResult);
    el.mergerResult.textContent = state.mergerResult
      ? t("mergerSuccess", state.mergerResult)
      : "";
  }
}

function render() {
  const splitterMode = state.mode === "splitter";
  el.modeSplitterBtn?.classList.toggle("active", splitterMode);
  el.modeMergerBtn?.classList.toggle("active", !splitterMode);
  el.modeSplitterBtn?.setAttribute("aria-selected", String(splitterMode));
  el.modeMergerBtn?.setAttribute("aria-selected", String(!splitterMode));
  el.splitterControls?.classList.toggle("hidden", !splitterMode);
  el.mergerControls?.classList.toggle("hidden", splitterMode);
  el.splitterWorkspace?.classList.toggle("hidden", !splitterMode);
  el.mergerWorkspace?.classList.toggle("hidden", splitterMode);

  updatePathLabel(el.packagePath, state.packagePath);
  updatePathLabel(el.destinationPath, state.destinationFolder);

  const visibleResources = getVisibleResources();
  el.selectedCount.textContent = t("selectedCount", state.selectedKeys.length);
  el.resultsSummary.textContent = state.packagePath
    ? [t("packageReady", visibleResources.length), state.manifestPackageCount ? t("manifestFound", state.manifestPackageCount) : ""]
        .filter(Boolean)
        .join(" • ")
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

  if (el.restoreManifestBtn) {
    el.restoreManifestBtn.classList.toggle("hidden", state.manifestPackageCount === 0);
  }

  renderContentFilter();
  renderList();
  renderPreview();
  renderMergerSources();
}

function renderList() {
  el.resourceList.innerHTML = "";
  const visibleResources = getVisibleResources();

  const duplicateCounts = new Map();
  for (const item of visibleResources) {
    const duplicateKey = String(item.label || "").trim().toLowerCase();
    if (!duplicateKey) continue;
    duplicateCounts.set(duplicateKey, (duplicateCounts.get(duplicateKey) || 0) + 1);
  }

  for (const item of visibleResources) {
    const displayType = getDisplayType(item);
    const displayDetails = getDisplayDetails(item);

    const duplicateKey = String(item.label || "").trim().toLowerCase();
    const duplicateCount = duplicateCounts.get(duplicateKey) || 0;
    const row = document.createElement("div");
    row.className =
      "resource-row" +
      (state.preview?.key === item.key ? " active" : "") +
      (duplicateCount > 1 ? " duplicate" : "");

    const checkWrap = document.createElement("div");
    checkWrap.className = "resource-check";

    const check = document.createElement("input");
    check.type = "checkbox";
    check.checked = state.selectedKeys.includes(item.key);
    check.addEventListener("click", (event) => handleSelectionClick(item.key, event));
    checkWrap.appendChild(check);

    const button = document.createElement("button");
    button.className = "resource-button";

    const metaLines = [];
    if (displayType) metaLines.push(displayType);
    if (displayDetails) metaLines.push(displayDetails);
    if (duplicateCount > 1) {
      metaLines.push(`<span class="duplicate-badge">${t("duplicateLabel", duplicateCount)}</span>`);
    }

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
  const keyParts = String(state.preview.key || "").split("-");
  const instance = keyParts.length >= 3 ? keyParts[2] : "";
  el.previewKey.textContent = instance ? `${t("instanceLabel")}: ${instance}` : "";
}

function handleSelectionClick(key, event) {
  const visibleResources = getVisibleResources();
  const targetChecked = event.currentTarget.checked;
  const currentIndex = visibleResources.findIndex((item) => item.key === key);
  const anchorIndex = state.selectionAnchorKey
    ? visibleResources.findIndex((item) => item.key === state.selectionAnchorKey)
    : -1;

  if (event.shiftKey && currentIndex >= 0 && anchorIndex >= 0) {
    const start = Math.min(anchorIndex, currentIndex);
    const end = Math.max(anchorIndex, currentIndex);
    const selected = new Set(state.selectedKeys);

    for (const item of visibleResources.slice(start, end + 1)) {
      if (targetChecked) {
        selected.add(item.key);
      } else {
        selected.delete(item.key);
      }
    }

    state.selectedKeys = [...selected];
  } else {
    if (targetChecked) {
      if (!state.selectedKeys.includes(key)) {
        state.selectedKeys = [...state.selectedKeys, key];
      }
    } else {
      state.selectedKeys = state.selectedKeys.filter((item) => item !== key);
    }

    state.selectionAnchorKey = key;
  }

  render();
}

async function pickPackage() {
  const result = await open({ multiple: false, filters: [{ name: "Package", extensions: ["package", "dbc"] }] });
  if (typeof result === "string") {
    state.packagePath = result;
    state.resources = [];
    state.selectedKeys = [];
    state.selectionInitialized = false;
    state.activeContentFilter = "all";
    state.isContentFilterOpen = false;
    state.selectionAnchorKey = null;
    state.preview = null;
    state.manifestPackageCount = 0;
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
    state.manifestPackageCount = result.manifestPackageCount || 0;

    if (!state.selectionInitialized) {
      state.selectedKeys = state.resources.map((item) => item.key);
      state.selectionInitialized = true;
    }

    state.selectionAnchorKey = null;
    state.preview = null;

    const firstVisible = getVisibleResources()[0] || state.resources[0];
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
    state.preview = preview;
    render();
  } catch (error) {
    console.error(error);
  }
}

function isMergerPackagePath(path) {
  const extension = String(path || "").toLowerCase().split(".").pop();
  return extension === "package" || extension === "dbc";
}

function addMergerSources(paths) {
  const existing = new Set(state.mergerSources.map(normalizeLocalPath));
  let added = 0;

  for (const path of Array.isArray(paths) ? paths : []) {
    if (!isMergerPackagePath(path)) continue;
    const normalized = normalizeLocalPath(path);
    if (!normalized || existing.has(normalized)) continue;
    existing.add(normalized);
    state.mergerSources.push(path);
    added += 1;
  }

  if (added > 0) state.mergerResult = null;
  render();
  return added;
}

async function collectMergerDroppedPaths(paths) {
  const droppedPaths = Array.isArray(paths) ? paths.filter(Boolean) : [];
  if (!droppedPaths.length) return;

  try {
    const packagePaths = await invoke("collect_merge_package_files", {
      payload: { paths: droppedPaths },
    });
    addMergerSources(packagePaths || []);
  } catch (error) {
    console.error(error);
    alert(`${t("mergerError")}\n${error}`);
  }
}

async function pickMergerPackages() {
  const result = await open({
    multiple: true,
    filters: [{ name: "Package", extensions: ["package", "dbc"] }],
  });
  const paths = Array.isArray(result) ? result : typeof result === "string" ? [result] : [];
  addMergerSources(paths);
}

function splitSavePath(outputPath) {
  const raw = String(outputPath || "").trim();
  const slash = Math.max(raw.lastIndexOf("/"), raw.lastIndexOf("\\"));
  if (slash < 0) return { destinationFolder: ".", outputName: raw };
  return {
    destinationFolder: raw.slice(0, slash),
    outputName: raw.slice(slash + 1),
  };
}

async function runMerger() {
  if (state.mergerBusy || state.mergerSources.length < 2) return;

  const outputPath = await save({
    defaultPath: "Merged.package",
    filters: [{ name: "The Sims 3 Package", extensions: ["package"] }],
  });
  if (typeof outputPath !== "string" || !outputPath.trim()) return;

  const { destinationFolder, outputName } = splitSavePath(outputPath);
  if (!outputName) return;

  state.mergerBusy = true;
  state.mergerResult = null;
  render();
  applyStaticTranslations();

  try {
    const result = await invoke("merge_packages_with_manifest", {
      payload: {
        packagePaths: state.mergerSources,
        destinationFolder,
        outputName,
      },
    });
    state.mergerResult = result;
    alert(t("mergerSuccess", result));
    await revealItemInDir(result.outputPath);
  } catch (error) {
    console.error(error);
    alert(`${t("mergerError")}\n${error}`);
  } finally {
    state.mergerBusy = false;
    applyStaticTranslations();
    render();
  }
}
async function restoreManifestPackages() {
  if (!state.packagePath || !state.manifestPackageCount) return;
  if (!state.destinationFolder) {
    alert(t("removeDestinationRequired"));
    return;
  }

  setBusy(t("restoreBusyTitle"), t("restoreBusyMessage"));
  try {
    const result = await invoke("restore_manifest_packages", {
      payload: {
        packagePath: state.packagePath,
        destinationFolder: state.destinationFolder,
      },
    });
    alert(t("restoreSuccess", result.packageCount));
    if (result.outputPaths?.[0]) {
      await revealItemInDir(result.outputPaths[0]);
    }
  } catch (error) {
    console.error(error);
    alert(`${t("restoreError")}\n${error}`);
  } finally {
    clearBusy();
  }
}

function clearMerger() {
  state.mergerSources = [];
  state.mergerResult = null;
  render();
}

function normalizeLocalPath(path) {
  return String(path || "").replaceAll("\\", "/").replace(/\/+$/, "").toLowerCase();
}

async function removeSelectionFromMerged() {
  if (!state.packagePath || !state.selectedKeys.length) return;
  if (!state.destinationFolder) {
    alert(t("removeDestinationRequired"));
    return;
  }

  try {
    const plan = await invoke("analyze_remove_selection", {
      payload: {
        packagePath: state.packagePath,
        selectedKeys: state.selectedKeys,
      },
    });

    if (!plan.resourcesToRemove && !plan.manifestPackagesToRemove) {
      alert(t("removeNothing"));
      return;
    }

    if (!window.confirm(t("removeConfirm", plan))) {
      return;
    }

    setBusy(t("removeBusyTitle"), t("removeBusyMessage"));

    const result = await invoke("remove_selected_from_merged", {
      payload: {
        packagePath: state.packagePath,
        destinationFolder: state.destinationFolder,
        selectedKeys: state.selectedKeys,
      },
    });

    alert(t("removeSuccess", result));
    await revealItemInDir(result.outputPath);

    if (normalizeLocalPath(result.outputPath) === normalizeLocalPath(state.packagePath)) {
      state.resources = [];
      state.selectedKeys = [];
      state.selectionInitialized = false;
      state.activeContentFilter = "all";
      state.isContentFilterOpen = false;
      state.selectionAnchorKey = null;
      state.preview = null;
      await scanPackage();
    }
  } catch (error) {
    console.error(error);
    alert(`${t("removeError")}\n${error}`);
  } finally {
    if (state.isBusy) {
      clearBusy();
    }
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

function clearAll() {
  state.packagePath = "";
  state.destinationFolder = "";
  state.resources = [];
  state.selectedKeys = [];
  state.selectionInitialized = false;
  state.activeContentFilter = "all";
  state.isContentFilterOpen = false;
  state.selectionAnchorKey = null;
  state.preview = null;
  state.manifestPackageCount = 0;
  state.search = "";
  if (el.searchInput) el.searchInput.value = "";
  render();
}

function bindDragAndDrop() {
  const splitterZone = el.packageDropZone;
  const mergerZone = el.mergerDropZone;

  const bindZoneVisuals = (zone) => {
    if (!zone) return;
    ["dragenter", "dragover"].forEach((eventName) => {
      zone.addEventListener(eventName, (event) => {
        event.preventDefault();
        zone.classList.add("drag-over");
      });
    });

    ["dragleave", "drop"].forEach((eventName) => {
      zone.addEventListener(eventName, (event) => {
        event.preventDefault();
        zone.classList.remove("drag-over");
      });
    });
  };

  bindZoneVisuals(splitterZone);
  bindZoneVisuals(mergerZone);

  let appWindow = null;
  try {
    appWindow = getCurrentWindow();
  } catch (error) {
    return;
  }

  appWindow.onDragDropEvent(async (event) => {
    const mergerMode = state.mode === "merger";
    const activeZone = mergerMode ? mergerZone : splitterZone;
    const inactiveZone = mergerMode ? splitterZone : mergerZone;

    if (event.payload.type === "over") {
      inactiveZone?.classList.remove("drag-over");
      activeZone?.classList.add("drag-over");
      return;
    }

    if (event.payload.type === "drop") {
      splitterZone?.classList.remove("drag-over");
      mergerZone?.classList.remove("drag-over");
      const paths = event.payload.paths || [];

      if (mergerMode) {
        await collectMergerDroppedPaths(paths);
        return;
      }

      const path = paths[0];
      const ext = path?.toLowerCase().split(".").pop();
      if (["package", "dbc"].includes(ext)) {
        state.packagePath = path;
        state.resources = [];
        state.selectedKeys = [];
        state.selectionInitialized = false;
        state.activeContentFilter = "all";
        state.isContentFilterOpen = false;
        state.selectionAnchorKey = null;
        state.preview = null;
        state.manifestPackageCount = 0;
        render();
      }
      return;
    }

    if (event.payload.type === "leave") {
      splitterZone?.classList.remove("drag-over");
      mergerZone?.classList.remove("drag-over");
    }
  });
}
function setupEvents() {
  el.modeSplitterBtn?.addEventListener("click", () => setMode("splitter"));
  el.modeMergerBtn?.addEventListener("click", () => setMode("merger"));
  el.mergerAddPackagesBtn?.addEventListener("click", pickMergerPackages);
  el.mergerRunBtn?.addEventListener("click", runMerger);
  el.mergerClearBtn?.addEventListener("click", clearMerger);
  el.restoreManifestBtn?.addEventListener("click", restoreManifestPackages);

  el.pickPackageBtn.addEventListener("click", pickPackage);
  el.pickDestinationBtn.addEventListener("click", pickDestination);
  el.scanBtn.addEventListener("click", scanPackage);
  el.exportBtn.addEventListener("click", () => exportSelection(false));
  if (el.removeMergedBtn) {
    el.removeMergedBtn.addEventListener("click", removeSelectionFromMerged);
  }

  if (el.zipBtn) {
    el.zipBtn.addEventListener("click", () => exportSelection(true));
  }

  el.clearBtn.addEventListener("click", clearAll);

  el.selectAllBtn.addEventListener("click", () => {
    state.selectedKeys = selectVisibleItems(
      state.resources,
      state.selectedKeys,
      state.activeContentFilter,
    );
    state.selectionInitialized = true;
    state.selectionAnchorKey = null;
    render();
  });

  el.selectNoneBtn.addEventListener("click", () => {
    state.selectedKeys = deselectVisibleItems(
      state.resources,
      state.selectedKeys,
      state.activeContentFilter,
    );
    state.selectionInitialized = true;
    state.selectionAnchorKey = null;
    render();
  });

  if (el.searchInput) {
    el.searchInput.addEventListener("input", (event) => {
      state.search = event.target.value;
      state.selectionAnchorKey = null;
      if (state.preview && !getVisibleResources().some((item) => item.key === state.preview.key)) {
        state.preview = null;
      }
      render();
    });
  }

  if (el.contentFilterToggleBtn) {
    el.contentFilterToggleBtn.addEventListener("click", (event) => {
      event.stopPropagation();
      state.isContentFilterOpen = !state.isContentFilterOpen;
      render();
    });
  }

  if (el.langToggleBtn) {
    el.langToggleBtn.addEventListener("click", (event) => {
      event.stopPropagation();
      state.isLangMenuOpen = !state.isLangMenuOpen;
      state.isContentFilterOpen = false;
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
    let changed = false;

    if (state.isLangMenuOpen && !el.langDropdown?.contains(event.target)) {
      state.isLangMenuOpen = false;
      changed = true;
    }

    if (state.isContentFilterOpen && !el.contentFilterDropdown?.contains(event.target)) {
      state.isContentFilterOpen = false;
      changed = true;
    }

    if (changed) {
      applyStaticTranslations();
      render();
    }
  });

  bindDragAndDrop();
}

setupEvents();
setLanguage(getSystemLanguage());
