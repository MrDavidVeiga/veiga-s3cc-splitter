export const CONTENT_FILTERS = [
  { value: "all", label: "All" },
  { value: "Hair", label: "Hair" },
  { value: "Facial Hair", label: "Facial Hair" },
  { value: "Eyebrows", label: "Eyebrows" },
  { value: "Body Hair", label: "Body Hair" },
  { value: "Clothes", label: "Clothes" },
  { value: "Shoes", label: "Shoes" },
  { value: "Accessories", label: "Accessories" },
  { value: "Makeup", label: "Makeup" },
  { value: "Eyes", label: "Eyes" },
  { value: "Skin Details", label: "Skin Details" },
  { value: "Tattoos", label: "Tattoos" },
  { value: "Teeth", label: "Teeth" },
  { value: "Face Parts", label: "Face Parts" },
  { value: "Pet Parts", label: "Pet Parts" },
  { value: "Pet Coats", label: "Pet Coats" },
  { value: "Pet Accessories", label: "Pet Accessories" },
  { value: "Other CASP", label: "Other CASP" },
  { value: "Sliders", label: "Sliders" },
  { value: "Skins", label: "Skins" },
  { value: "Pose", label: "Pose" },
  { value: "Clip", label: "Clip" },
  { value: "Comfort", label: "Comfort" },
  { value: "Surfaces", label: "Surfaces" },
  { value: "Storage", label: "Storage" },
  { value: "Appliances", label: "Appliances" },
  { value: "Electronics", label: "Electronics" },
  { value: "Plumbing", label: "Plumbing" },
  { value: "Lighting", label: "Lighting" },
  { value: "Decor", label: "Decor" },
  { value: "Entertainment", label: "Entertainment" },
  { value: "Kids", label: "Kids" },
  { value: "Vehicles", label: "Vehicles" },
  { value: "Pets", label: "Pets" },
  { value: "Show Stage", label: "Show Stage" },
  { value: "Resort", label: "Resort" },
  { value: "Debug", label: "Debug" },
  { value: "Other Objects", label: "Other Objects" },
  { value: "Doors & Arches", label: "Doors & Arches" },
  { value: "Windows", label: "Windows" },
  { value: "Fences & Gates", label: "Fences & Gates" },
  { value: "Stairs & Railings", label: "Stairs & Railings" },
  { value: "Columns", label: "Columns" },
  { value: "Foundations", label: "Foundations" },
  { value: "Walls", label: "Walls" },
  { value: "Floors", label: "Floors" },
  { value: "Ceilings", label: "Ceilings" },
  { value: "Roofs", label: "Roofs" },
  { value: "Fireplaces & Chimneys", label: "Fireplaces & Chimneys" },
  { value: "Plants & Rocks", label: "Plants & Rocks" },
  { value: "Terrain Paints", label: "Terrain Paints" },
  { value: "Terrain Tools", label: "Terrain Tools" },
  { value: "Pools & Fountains", label: "Pools & Fountains" },
  { value: "Elevators", label: "Elevators" },
  { value: "Rabbit Holes", label: "Rabbit Holes" },
  { value: "Building Shells", label: "Building Shells" },
  { value: "Landmarks", label: "Landmarks" },
  { value: "Blueprints", label: "Blueprints" },
  { value: "Resort Objects", label: "Resort Objects" },
  { value: "Other Build", label: "Other Build" },
];

const CONTENT_FILTER_VALUES = CONTENT_FILTERS
  .filter((filter) => filter.value !== "all")
  .map((filter) => filter.value);

const CASP_PRIORITY = [
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
];

const SPECIAL_PRIORITY = ["Sliders", "Skins", "Pose", "Clip"];

const OBJECT_PRIORITY = [
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
];

const BUILD_PRIORITY = [
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
];

const TECHNICAL_FALLBACK_CATEGORIES = {
  CASP: "Other CASP",
  FACE: "Sliders",
  SkinTone: "Skins",
  OBJD: "Other Objects",
  CLIP: "Clip",
};

function uniqueKnownCategories(item) {
  const rawCategories = Array.isArray(item?.contentCategories) ? item.contentCategories : [];
  const categories = [];
  for (const category of rawCategories) {
    if (!CONTENT_FILTER_VALUES.includes(category) || categories.includes(category)) continue;
    categories.push(category);
  }
  return categories;
}

function firstCategory(categories, priority) {
  return priority.find((category) => categories.includes(category)) || null;
}

export function primaryContentCategory(item) {
  const categories = uniqueKnownCategories(item);

  const caspSpecific = firstCategory(categories, CASP_PRIORITY);
  if (caspSpecific) return caspSpecific;
  if (categories.includes("Other CASP")) return "Other CASP";

  const special = firstCategory(categories, SPECIAL_PRIORITY);
  if (special) return special;

  const objectSpecific = firstCategory(categories, OBJECT_PRIORITY);
  if (objectSpecific) return objectSpecific;
  if (categories.includes("Other Objects")) return "Other Objects";

  const buildSpecific = firstCategory(categories, BUILD_PRIORITY);
  if (buildSpecific) return buildSpecific;
  if (categories.includes("Other Build")) return "Other Build";

  const rawFallbackTypes = [item?.kind, item?.type, item?.resourceType].filter(Boolean);
  for (const type of rawFallbackTypes) {
    const category = TECHNICAL_FALLBACK_CATEGORIES[type];
    if (category) return category;
  }

  return null;
}

export function normalizeItemContentTypes(item) {
  const category = primaryContentCategory(item);
  return category ? [category] : [];
}

export function itemMatchesContentFilter(item, filterValue) {
  return filterValue === "all" || normalizeItemContentTypes(item).includes(filterValue);
}

export function visibleItemsForFilter(items, filterValue) {
  return items.filter((item) => itemMatchesContentFilter(item, filterValue));
}

export function contentFilterCounts(items) {
  const counts = { all: items.length };
  for (const type of CONTENT_FILTER_VALUES) counts[type] = 0;

  for (const item of items) {
    for (const type of normalizeItemContentTypes(item)) {
      counts[type] += 1;
    }
  }
  return counts;
}

export function availableContentFilters(items) {
  const counts = contentFilterCounts(items);
  return CONTENT_FILTERS.filter((filter) => filter.value === "all" || counts[filter.value] > 0);
}

export function selectVisibleItems(items, selectedKeys, filterValue) {
  const selected = new Set(selectedKeys);
  for (const item of visibleItemsForFilter(items, filterValue)) {
    selected.add(item.key);
  }
  return Array.from(selected);
}

export function deselectVisibleItems(items, selectedKeys, filterValue) {
  const visibleKeys = new Set(visibleItemsForFilter(items, filterValue).map((item) => item.key));
  return selectedKeys.filter((key) => !visibleKeys.has(key));
}
