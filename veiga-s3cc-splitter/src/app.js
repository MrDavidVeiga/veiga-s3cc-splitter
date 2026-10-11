import "@fortawesome/fontawesome-free/css/all.min.css";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import noPreviewImage from "./assets/no-preview.png";
import exportIcon from "./assets/icons/export.png";
import zipIcon from "./assets/icons/zip.png";
import clearIcon from "./assets/icons/clear.png";
import { canUseReversibleMergeActions, formatRemoveConfirm } from "./remove-confirm.js";
import { createModalController, runConfirmedAction } from "./modal.js";
import {
  availableContentFilters,
  CONTENT_FILTERS,
  contentFilterCounts,
  deselectVisibleItems,
  selectVisibleItems,
  visibleItemsForFilter,
} from "./filter-logic.js";
import { buildSameNameCounts, displayRelationForItem, sameNameCountForItem } from "./label-relations.js";

const countForm = (value, singular, plural) => Number(value) === 1 ? singular : plural;

const translations = {
  en: {
    appTitle: "Veiga's S3CC Splitter",
    langLabel: "Language:",
    pickPackage: "Open Package",
    pickPackageTip: "Select a .package or .dbc file to open in the Splitter.",
    dropHint: "Select or drag a .package or .dbc here",
    scanList: "Load List",
    modeSplitter: "Splitter",
    modeMerger: "Merger",
    mergerAddPackages: "Add Packages",
    mergerAddHint: "Select .package/.dbc files or drag files/folders here",
    mergerRun: "Merge Packages",
    mergerClear: "Clear List",
    mergerTitle: "Packages to Merge",
    mergerNoPackages: "No packages selected.",
    mergerCount: (count) => `${count} package${count === 1 ? "" : "s"} selected.`,
    mergerBusy: "Merging packages...",
    mergerSuccess: (result) => `Merge completed: ${result.packageCount} ${countForm(result.packageCount, "original package", "original packages")}, ${result.resourceCount} ${countForm(result.resourceCount, "unique resource", "unique resources")}, ${result.sharedResources} ${countForm(result.sharedResources, "shared resource", "shared resources")}.`,
    mergerError: "Merge failed:",
    restoreOriginals: "Unmerge",
    restoreBusyTitle: "Unmerging...",
    restoreBusyMessage: "Rebuilding the original packages from the embedded Merge Manifest.",
    restoreSuccess: (result) => {
      const rebuilt = Number(result?.packageCount || 0);
      const duplicates = Number(result?.duplicateCount || 0);
      const message = `Unmerge completed successfully. ${rebuilt} ${countForm(rebuilt, "original package", "original packages")} rebuilt.`;
      if (!duplicates) return message;
      return `${message}\n${duplicates} ${countForm(duplicates, "duplicate package already existed in the destination and was not recreated", "duplicate packages already existed in the destination and were not recreated")}.`;
    },
    restoreError: "Failed to unmerge:",
    manifestFound: (count) => `Merge Manifest: ${count} ${countForm(count, "original package", "original packages")}`,
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
      "Pose": "Pose",
      "Clip": "Clip",
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
    duplicateHint: "Verified duplicate logical content. Names alone are never used to make this classification.",
    tgiConflictLabel: (count) => `TGI conflict ×${count}`,
    tgiConflictHint: "The same TGI appears with different payload data in this item or one of its dependencies.",
    sameNameLabel: (count) => `Same name ×${count}`,
    sameNameHint: "These items share the same internal/display name, but they have not been identified as duplicate content.",
    resourcesTitle: "Resources",
    noPackageLoaded: "No package loaded.",
    packageReady: (count) => `${count} found:`,
    selectedCount: (count) => `${count} selected`,
    selectAll: "Select All",
    selectNone: "Select None",
    busyDefaultTitle: "Processing",
    busyDefaultMessage: "Please wait.",
    busyReadTitle: "Reading package...",
    busyReadMessage: "Building the resource list from the file.",
    busyExportTitle: "Exporting...",
    busyExportMessage: "Saving resources to folder.",
    modalInfoTitle: "Information",
    modalErrorTitle: "Error",
    modalWarningTitle: "Warning",
    modalSuccessTitle: "Success",
    modalOk: "OK",
    modalCancel: "Cancel",
    modalClose: "Close",
    modalRemove: "Remove",
    emptyTitle: "Open a package to begin.",
    emptySubtitle: "The list, preview and actions will appear here.",
    previewTitle: "Preview",
    revealSourcePackage: "Reveal source package in folder",
    previewEmpty: "Select a resource to see the preview.",
    exportSelection: "Export Selection",
    removeFromMerged: "Remove from Merged",
    removeSelectedFiles: "Remove Selected Files",
    removeBusyTitle: "Rebuilding merged package...",
    removeBusyMessage: "Preserving shared resources and validating the new package.",
    removeDestinationRequired: "Choose a destination folder first.",
    removeNothing: "None of the selected resources can be removed safely.",
    removeConfirm: (plan) => formatRemoveConfirm("en", plan),
    removeSuccess: (result) => `Merged package rebuilt successfully.\n\n${countForm(result.removedResources, "Removed resource", "Removed resources")}: ${result.removedResources}\n${countForm(result.removedManifestPackages || 0, "Manifest package entry removed", "Manifest package entries removed")}: ${result.removedManifestPackages || 0}\n${countForm(result.sharedResourcesPreserved, "Shared resource preserved", "Shared resources preserved")}: ${result.sharedResourcesPreserved}\nBackup: ${result.backupPath}`,
    removeError: "Failed to remove selected items:",
    clear: "Clear List",
    readError: "Failed to read package:",
    splittError: "Export failed:",
  },
  pt: {
    appTitle: "Veiga's S3CC Splitter",
    langLabel: "Idioma:",
    pickPackage: "Abrir Package",
    pickPackageTip: "Selecione um arquivo .package ou .dbc para abrir no Separador.",
    dropHint: "Selecione ou arraste um .package ou .dbc aqui",
    scanList: "Carregar Lista",
    modeSplitter: "Separador",
    modeMerger: "Mesclador",
    mergerAddPackages: "Adicionar Packages",
    mergerAddHint: "Selecione .package/.dbc ou arraste arquivos/pastas aqui",
    mergerRun: "Mesclar Packages",
    mergerClear: "Limpar Lista",
    mergerTitle: "Packages para Mesclar",
    mergerNoPackages: "Nenhum package selecionado.",
    mergerCount: (count) => `${count} package${count === 1 ? "" : "s"} selecionado${count === 1 ? "" : "s"}.`,
    mergerBusy: "Mesclando packages...",
    mergerSuccess: (result) => `Mesclagem concluída: ${result.packageCount} ${countForm(result.packageCount, "package original", "packages originais")}, ${result.resourceCount} ${countForm(result.resourceCount, "recurso único", "recursos únicos")}, ${result.sharedResources} ${countForm(result.sharedResources, "recurso compartilhado", "recursos compartilhados")}.`,
    mergerError: "Falha ao mesclar:",
    restoreOriginals: "Desmesclar",
    restoreBusyTitle: "Desmesclando...",
    restoreBusyMessage: "Reconstruindo os packages a partir do Merge Manifest interno.",
    restoreSuccess: (result) => {
      const rebuilt = Number(result?.packageCount || 0);
      const duplicates = Number(result?.duplicateCount || 0);
      const message = `Desmesclagem concluída com sucesso. ${rebuilt} ${countForm(rebuilt, "package original reconstruído", "packages originais reconstruídos")}.`;
      if (!duplicates) return message;
      return `${message}\n${duplicates} ${countForm(duplicates, "package duplicado já existia no destino e não foi recriado", "packages duplicados já existiam no destino e não foram recriados")}.`;
    },
    restoreError: "Falha ao desmesclar:",
    manifestFound: (count) => `Merge Manifest: ${count} ${countForm(count, "package original", "packages originais")}`,
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
      "Pose": "Pose",
      "Clip": "Clip",
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
    instanceLabel: "Instância",
    duplicateLabel: (count) => `Duplicado ×${count}`,
    duplicateHint: "Conteúdo lógico duplicado verificado. O nome sozinho nunca é usado para esta classificação.",
    tgiConflictLabel: (count) => `Conflito de TGI ×${count}`,
    tgiConflictHint: "O mesmo TGI aparece com dados diferentes neste item ou em uma de suas dependências.",
    sameNameLabel: (count) => `Mesmo nome ×${count}`,
    sameNameHint: "Estes itens usam o mesmo nome interno/exibido, mas não foram identificados como conteúdo duplicado.",
    resourcesTitle: "Recursos",
    noPackageLoaded: "Nenhum package carregado.",
    packageReady: (count) => `${count} ${count === 1 ? "encontrado" : "encontrados"}:`,
    selectedCount: (count) => `${count} ${countForm(count, "selecionado", "selecionados")}`,
    selectAll: "Selecionar Tudo",
    selectNone: "Selecionar Nenhum",
    busyDefaultTitle: "Processando",
    busyDefaultMessage: "Aguarde.",
    busyReadTitle: "Lendo package...",
    busyReadMessage: "Montando a lista de recursos do arquivo.",
    busyExportTitle: "Exportando...",
    busyExportMessage: "Gravando recursos em pasta.",
    modalInfoTitle: "Informação",
    modalErrorTitle: "Erro",
    modalWarningTitle: "Aviso",
    modalSuccessTitle: "Sucesso",
    modalOk: "OK",
    modalCancel: "Cancelar",
    modalClose: "Fechar",
    modalRemove: "Remover",
    emptyTitle: "Abra um package para começar.",
    emptySubtitle: "A lista, preview e ações aparecem aqui.",
    previewTitle: "Pré-visualização",
    revealSourcePackage: "Mostrar package de origem na pasta",
    previewEmpty: "Selecione um recurso para ver o preview.",
    exportSelection: "Exportar Seleção",
    removeFromMerged: "Remover do Mesclado",
    removeSelectedFiles: "Remover Arquivos Selecionados",
    removeBusyTitle: "Reconstruindo package mesclado...",
    removeBusyMessage: "Preservando recursos compartilhados e validando o novo package.",
    removeDestinationRequired: "Escolha primeiro uma pasta de destino.",
    removeNothing: "Nenhum dos recursos selecionados pode ser removido com segurança.",
    removeConfirm: (plan) => formatRemoveConfirm("pt", plan),
    removeSuccess: (result) => `Package mesclado reconstruído com sucesso.\n\n${countForm(result.removedResources, "Recurso removido", "Recursos removidos")}: ${result.removedResources}\n${countForm(result.removedManifestPackages || 0, "Entrada removida do manifesto", "Entradas removidas do manifesto")}: ${result.removedManifestPackages || 0}\n${countForm(result.sharedResourcesPreserved, "Recurso compartilhado preservado", "Recursos compartilhados preservados")}: ${result.sharedResourcesPreserved}\nBackup: ${result.backupPath}`,
    removeError: "Falha ao remover os itens selecionados:",
    clear: "Limpar Lista",
    readError: "Falha ao ler package:",
    splittError: "Falha na exportação:",
  },
  es: {
    appTitle: "Veiga's S3CC Splitter",
    langLabel: "Idioma:",
    pickPackage: "Abrir Package",
    pickPackageTip: "Selecciona un archivo .package o .dbc para abrirlo en el Separador.",
    dropHint: "Selecciona o arrastra un .package o .dbc aquí",
    scanList: "Cargar Lista",
    modeSplitter: "Separador",
    modeMerger: "Fusionador",
    mergerAddPackages: "Añadir Packages",
    mergerAddHint: "Selecciona .package/.dbc o arrastra archivos/carpetas aquí",
    mergerRun: "Fusionar Packages",
    mergerClear: "Limpiar Lista",
    mergerTitle: "Packages para Fusionar",
    mergerNoPackages: "Ningún package seleccionado.",
    mergerCount: (count) => `${count} package${count === 1 ? "" : "s"} seleccionado${count === 1 ? "" : "s"}.`,
    mergerBusy: "Fusionando packages...",
    mergerSuccess: (result) => `Fusión completada: ${result.packageCount} ${countForm(result.packageCount, "package original", "packages originales")}, ${result.resourceCount} ${countForm(result.resourceCount, "recurso único", "recursos únicos")}, ${result.sharedResources} ${countForm(result.sharedResources, "recurso compartido", "recursos compartidos")}.`,
    mergerError: "Error al fusionar:",
    restoreOriginals: "Desfusionar",
    restoreBusyTitle: "Desfusionando...",
    restoreBusyMessage: "Reconstruyendo los packages desde el Merge Manifest interno.",
    restoreSuccess: (result) => {
      const rebuilt = Number(result?.packageCount || 0);
      const duplicates = Number(result?.duplicateCount || 0);
      const message = `Desfusión completada correctamente. ${rebuilt} ${countForm(rebuilt, "package original reconstruido", "packages originales reconstruidos")}.`;
      if (!duplicates) return message;
      return `${message}\n${duplicates} ${countForm(duplicates, "package duplicado ya existía en el destino y no se volvió a crear", "packages duplicados ya existían en el destino y no se volvieron a crear")}.`;
    },
    restoreError: "Error al desfusionar:",
    manifestFound: (count) => `Merge Manifest: ${count} ${countForm(count, "package original", "packages originales")}`,
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
      "Pose": "Pose",
      "Clip": "Clip",
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
    instanceLabel: "Instancia",
    duplicateLabel: (count) => `Duplicado ×${count}`,
    duplicateHint: "Contenido lógico duplicado verificado. El nombre por sí solo nunca se usa para esta clasificación.",
    tgiConflictLabel: (count) => `Conflicto TGI ×${count}`,
    tgiConflictHint: "El mismo TGI aparece con datos diferentes en este elemento o en una de sus dependencias.",
    sameNameLabel: (count) => `Mismo nombre ×${count}`,
    sameNameHint: "Estos elementos usan el mismo nombre interno/visible, pero no han sido identificados como contenido duplicado.",
    resourcesTitle: "Recursos",
    noPackageLoaded: "Ningún package cargado.",
    packageReady: (count) => `${count} ${count === 1 ? "encontrado" : "encontrados"}:`,
    selectedCount: (count) => `${count} ${countForm(count, "seleccionado", "seleccionados")}`,
    selectAll: "Seleccionar Todo",
    selectNone: "Deseleccionar Todo",
    busyDefaultTitle: "Procesando",
    busyDefaultMessage: "Espera un momento.",
    busyReadTitle: "Leyendo package...",
    busyReadMessage: "Armando la lista de recursos del archivo.",
    busyExportTitle: "Exportando...",
    busyExportMessage: "Guardando recursos en la carpeta.",
    modalInfoTitle: "Información",
    modalErrorTitle: "Error",
    modalWarningTitle: "Aviso",
    modalSuccessTitle: "Éxito",
    modalOk: "OK",
    modalCancel: "Cancelar",
    modalClose: "Cerrar",
    modalRemove: "Eliminar",
    emptyTitle: "Abre un package para empezar.",
    emptySubtitle: "La lista, la vista previa y las acciones aparecerán aquí.",
    previewTitle: "Vista previa",
    revealSourcePackage: "Mostrar package de origen en la carpeta",
    previewEmpty: "Selecciona un recurso para ver la vista previa.",
    exportSelection: "Exportar Selección",
    removeFromMerged: "Eliminar del Fusionado",
  removeSelectedFiles: "Eliminar Archivos Seleccionados",
    removeBusyTitle: "Reconstruyendo package fusionado...",
    removeBusyMessage: "Conservando recursos compartidos y validando el nuevo package.",
    removeDestinationRequired: "Elige primero una carpeta de destino.",
    removeNothing: "Ninguno de los recursos seleccionados puede eliminarse de forma segura.",
    removeConfirm: (plan) => formatRemoveConfirm("es", plan),
    removeSuccess: (result) => `Package fusionado reconstruido correctamente.\n\n${countForm(result.removedResources, "Recurso eliminado", "Recursos eliminados")}: ${result.removedResources}\n${countForm(result.removedManifestPackages || 0, "Entrada eliminada del manifiesto", "Entradas eliminadas del manifiesto")}: ${result.removedManifestPackages || 0}\n${countForm(result.sharedResourcesPreserved, "Recurso compartido conservado", "Recursos compartidos conservados")}: ${result.sharedResourcesPreserved}\nBackup: ${result.backupPath}`,
    removeError: "Error al eliminar los elementos seleccionados:",
    clear: "Limpiar Lista",
    readError: "Error al leer el package:",
    splittError: "Error en la exportación:",
  },
};

const state = {
  mode: "splitter",
  packagePath: "",
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
  scanBtn: document.querySelector("#scan-btn"),
  exportBtn: document.querySelector("#export-btn"),
  removeMergedBtn: document.querySelector("#remove-merged-btn"),
  clearBtn: document.querySelector("#clear-btn"),
  selectAllBtn: document.querySelector("#select-all-btn"),
  selectNoneBtn: document.querySelector("#select-none-btn"),
  packageDropZone: document.querySelector("#package-drop-zone"),
  packagePath: document.querySelector("#package-path"),
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
  modalRoot: document.querySelector("#app-modal"),
  modalPanel: document.querySelector("#app-modal-panel"),
  modalTitle: document.querySelector("#app-modal-title"),
  modalMessage: document.querySelector("#app-modal-message"),
  modalCloseBtn: document.querySelector("#app-modal-close"),
  modalCancelBtn: document.querySelector("#app-modal-cancel"),
  modalPrimaryBtn: document.querySelector("#app-modal-primary"),
};

const appModal = createModalController({
  root: el.modalRoot,
  panel: el.modalPanel,
  title: el.modalTitle,
  message: el.modalMessage,
  closeButton: el.modalCloseBtn,
  cancelButton: el.modalCancelBtn,
  primaryButton: el.modalPrimaryBtn,
}, {
  getCloseLabel: () => t("modalClose"),
});

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
  if (el.pickPackageBtn) {
    el.pickPackageBtn.textContent = t("pickPackage");
    el.pickPackageBtn.title = t("pickPackageTip");
  }
  // Update only the text, preserving the drag target icon and .package/.dbc list.
  if (el.dropHint) el.dropHint.querySelector(".drop-hint-title")?.replaceChildren(t("dropHint"));
  if (el.scanBtn) el.scanBtn.textContent = t("scanList");
  if (el.modeSplitterBtn) el.modeSplitterBtn.textContent = t("modeSplitter");
  if (el.modeMergerBtn) el.modeMergerBtn.textContent = t("modeMerger");
  if (el.mergerAddPackagesBtn) el.mergerAddPackagesBtn.textContent = t("mergerAddPackages");
  if (el.mergerAddHint) el.mergerAddHint.querySelector(".drop-hint-title")?.replaceChildren(t("mergerAddHint"));
  if (el.mergerRunBtn) el.mergerRunBtn.innerHTML = `<img src="${zipIcon}" alt="" class="btn-icon" /><span>${state.mergerBusy ? t("mergerBusy") : t("mergerRun")}</span>`;
  if (el.mergerClearBtn) el.mergerClearBtn.innerHTML = `<img src="${clearIcon}" alt="" class="btn-icon" /><span>${t("mergerClear")}</span>`;
  if (el.mergerTitle) el.mergerTitle.textContent = t("mergerTitle");
  if (el.restoreManifestBtn) {
    el.restoreManifestBtn.innerHTML = `<i class="fa-solid fa-rotate-left" aria-hidden="true"></i><span>${t("restoreOriginals")}</span>`;
    el.restoreManifestBtn.title = t("restoreOriginals");
    el.restoreManifestBtn.setAttribute("aria-label", t("restoreOriginals"));
  }
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
  if (el.removeMergedBtn) {
    const removalLabel = t("removeFromMerged");
    el.removeMergedBtn.innerHTML = `<img src="${clearIcon}" alt="" class="btn-icon" /><span>${removalLabel}</span>`;
    el.removeMergedBtn.title = removalLabel;
    el.removeMergedBtn.setAttribute("aria-label", removalLabel);
  }
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

  // Carregar Lista is available only after a valid source package is chosen.
  if (el.scanBtn) el.scanBtn.disabled = state.isBusy || !isScannablePackagePath(state.packagePath);
  updatePathLabel(el.packagePath, state.packagePath);

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

  const hasValidMergeManifest = canUseReversibleMergeActions(state.manifestPackageCount);
  if (el.restoreManifestBtn) {
    el.restoreManifestBtn.classList.toggle("hidden", !hasValidMergeManifest);
    el.restoreManifestBtn.disabled = !hasValidMergeManifest;
  }
  if (el.removeMergedBtn) {
    el.removeMergedBtn.classList.remove("hidden");
    el.removeMergedBtn.disabled = state.selectedKeys.length === 0;
    const removalLabel = t("removeFromMerged");
    el.removeMergedBtn.innerHTML = `<img src="${clearIcon}" alt="" class="btn-icon" /><span>${removalLabel}</span>`;
    el.removeMergedBtn.title = removalLabel;
    el.removeMergedBtn.setAttribute("aria-label", removalLabel);
  }

  renderContentFilter();
  renderList();
  renderPreview();
  renderMergerSources();
}

function renderList() {
  el.resourceList.innerHTML = "";
  const visibleResources = getVisibleResources();

  const sameNameCounts = buildSameNameCounts(visibleResources);

  for (const item of visibleResources) {
    const displayType = getDisplayType(item);
    const displayDetails = getDisplayDetails(item);

    const sameNameCount = sameNameCountForItem(item, sameNameCounts);
    const relation = displayRelationForItem(item, sameNameCount);
    const row = document.createElement("div");
    row.className =
      "resource-row" +
      (state.preview?.key === item.key ? " active" : "") +
      (relation ? ` ${relation.kind}` : "");

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
    if (relation?.kind === "tgi-conflict") {
      metaLines.push(
        `<span class="tgi-conflict-badge" title="${t("tgiConflictHint")}">${t("tgiConflictLabel", relation.count)}</span>`,
      );
    } else if (relation?.kind === "duplicate") {
      metaLines.push(
        `<span class="duplicate-badge" title="${t("duplicateHint")}">${t("duplicateLabel", relation.count)}</span>`,
      );
    } else if (relation?.kind === "same-name") {
      metaLines.push(
        `<span class="same-name-badge" title="${t("sameNameHint")}">${t("sameNameLabel", relation.count)}</span>`,
      );
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
  const canRevealSource = Boolean(state.preview && state.packagePath);
  el.previewThumb.classList.toggle("reveal-source", canRevealSource);
  el.previewThumb.draggable = false;
  el.previewThumb.tabIndex = canRevealSource ? 0 : -1;
  el.previewThumb.setAttribute("role", canRevealSource ? "button" : "presentation");
  el.previewThumb.setAttribute("aria-label", canRevealSource ? t("revealSourcePackage") : "");
  el.previewThumb.title = canRevealSource ? state.packagePath : "";
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

function isScannablePackagePath(path) {
  return /\.(?:package|dbc)$/i.test(String(path || "").trim());
}

async function pickPackage() {
  const result = await open({ multiple: false, filters: [{ name: "Package", extensions: ["package", "dbc"] }] });
  if (typeof result === "string" && isScannablePackagePath(result)) {
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

async function chooseOutputFolder() {
  const result = await open({ directory: true, multiple: false });
  return typeof result === "string" && result.trim() ? result : null;
}

function parentFolderOfPath(path) {
  const raw = String(path || "").trim();
  const slash = Math.max(raw.lastIndexOf("/"), raw.lastIndexOf("\\"));
  if (slash < 0) return ".";
  return raw.slice(0, slash) || ".";
}

async function scanPackage() {
  if (state.isBusy || !isScannablePackagePath(state.packagePath)) return;

  setBusy(t("busyReadTitle"), t("busyReadMessage"));

  try {
    const result = await invoke("scan_package_resources", {
      payload: {
        packagePath: state.packagePath,
        resourceType: "all",
        search: null,
      },
    });

    const relations = result.relations || {};
    state.resources = (result.items || []).map((item) => ({
      ...item,
      relation: relations[item.key] || null,
    }));
    state.manifestPackageCount = result.manifestPackageCount || 0;

    state.selectedKeys = [];
    state.selectionInitialized = true;

    state.selectionAnchorKey = null;
    state.preview = null;

    const firstVisible = getVisibleResources()[0] || state.resources[0];
    if (firstVisible) {
      await loadPreview(firstVisible);
    }
  } catch (error) {
    console.error(error);
    await appModal.showError({
      titleText: t("modalErrorTitle"),
      messageText: `${t("readError")}\n${error}`,
      primaryLabel: t("modalClose"),
    });
  } finally {
    clearBusy();
  }
}

async function revealPreviewSource() {
  if (!state.preview || !state.packagePath) return;
  try {
    // A resource is embedded; reveal its physical source package, selected in Explorer.
    await revealItemInDir(state.packagePath);
  } catch (error) {
    console.error(error);
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
    await appModal.showError({
      titleText: t("modalErrorTitle"),
      messageText: `${t("mergerError")}\n${error}`,
      primaryLabel: t("modalClose"),
    });
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
    await appModal.showSuccess({
      titleText: t("modalSuccessTitle"),
      messageText: t("mergerSuccess", result),
      primaryLabel: t("modalOk"),
    });
    await revealItemInDir(result.outputPath);
  } catch (error) {
    console.error(error);
    await appModal.showError({
      titleText: t("modalErrorTitle"),
      messageText: `${t("mergerError")}\n${error}`,
      primaryLabel: t("modalClose"),
    });
  } finally {
    state.mergerBusy = false;
    applyStaticTranslations();
    render();
  }
}
async function restoreManifestPackages() {
  if (!state.packagePath || !state.manifestPackageCount) return;

  const destinationFolder = await chooseOutputFolder();
  if (!destinationFolder) return;

  setBusy(t("restoreBusyTitle"), t("restoreBusyMessage"));
  try {
    const result = await invoke("restore_manifest_packages", {
      payload: {
        packagePath: state.packagePath,
        destinationFolder,
      },
    });
    await appModal.showSuccess({
      titleText: t("modalSuccessTitle"),
      messageText: t("restoreSuccess", result),
      primaryLabel: t("modalOk"),
    });
    const revealPath = result.outputPaths?.[0] || result.duplicatePaths?.[0];
    if (revealPath) {
      await revealItemInDir(revealPath);
    }
  } catch (error) {
    console.error(error);
    await appModal.showError({
      titleText: t("modalErrorTitle"),
      messageText: `${t("restoreError")}\n${error}`,
      primaryLabel: t("modalClose"),
    });
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

  const destinationFolder = parentFolderOfPath(state.packagePath);

  try {
    const plan = await invoke("analyze_remove_selection", {
      payload: {
        packagePath: state.packagePath,
        selectedKeys: state.selectedKeys,
      },
    });

    if (!plan.resourcesToRemove && !plan.manifestPackagesToRemove) {
      await appModal.showWarning({
        titleText: t("modalWarningTitle"),
        messageText: t("removeNothing"),
        primaryLabel: t("modalOk"),
      });
      return;
    }

    const confirmPlan = { ...plan, logicalItemTotal: state.resources.length };
    const removal = await runConfirmedAction(
      () => appModal.showConfirm({
        titleText: t("removeFromMerged"),
        messageText: t("removeConfirm", confirmPlan),
        cancelLabel: t("modalCancel"),
        primaryLabel: t("modalRemove"),
        danger: true,
      }),
      async () => {
        setBusy(t("removeBusyTitle"), t("removeBusyMessage"));
        return invoke("remove_selected_from_merged", {
          payload: {
            packagePath: state.packagePath,
            destinationFolder,
            selectedKeys: state.selectedKeys,
          },
        });
      },
    );

    if (!removal.confirmed) {
      return;
    }

    const result = removal.result;
    await appModal.showSuccess({
      titleText: t("modalSuccessTitle"),
      messageText: t("removeSuccess", result),
      primaryLabel: t("modalOk"),
    });
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
    await appModal.showError({
      titleText: t("modalErrorTitle"),
      messageText: `${t("removeError")}\n${error}`,
      primaryLabel: t("modalClose"),
    });
  } finally {
    if (state.isBusy) {
      clearBusy();
    }
  }
}

async function exportSelection() {
  if (!state.packagePath || !state.selectedKeys.length) return;

  const destinationFolder = await chooseOutputFolder();
  if (!destinationFolder) return;

  const payload = {
    packagePath: state.packagePath,
    destinationFolder,
    selectedKeys: state.selectedKeys,
    includeThumbnails: el.optThumbs.checked,
    enableCompression: el.optCompression.checked,
    createClonePackage: el.optClone.checked,
  };

  setBusy(t("busyExportTitle"), t("busyExportMessage"));

  try {
    const outputPath = await invoke("export_selected_resources", { payload });
    await revealItemInDir(outputPath);
  } catch (error) {
    console.error(error);
    await appModal.showError({
      titleText: t("modalErrorTitle"),
      messageText: `${t("splittError")}\n${error}`,
      primaryLabel: t("modalClose"),
    });
  } finally {
    clearBusy();
  }
}

function clearAll() {
  state.packagePath = "";
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
  el.previewThumb.addEventListener("click", () => void revealPreviewSource());
  el.previewThumb.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    if (!state.preview || !state.packagePath) return;
    event.preventDefault();
    void revealPreviewSource();
  });

  el.pickPackageBtn.addEventListener("click", pickPackage);
  el.scanBtn.addEventListener("click", scanPackage);
  el.exportBtn.addEventListener("click", exportSelection);
  if (el.removeMergedBtn) {
    el.removeMergedBtn.addEventListener("click", removeSelectionFromMerged);
  }

  el.clearBtn.addEventListener("click", clearAll);

  el.selectAllBtn.addEventListener("click", () => {
    state.selectedKeys = selectVisibleItems(getVisibleResources(), state.selectedKeys, "all");
    state.selectionInitialized = true;
    state.selectionAnchorKey = null;
    render();
  });

  el.selectNoneBtn.addEventListener("click", () => {
    state.selectedKeys = deselectVisibleItems(getVisibleResources(), state.selectedKeys, "all");
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
