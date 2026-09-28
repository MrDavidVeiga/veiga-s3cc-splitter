import assert from "node:assert/strict";
import {
  availableContentFilters,
  contentFilterCounts,
  deselectVisibleItems,
  itemMatchesContentFilter,
  normalizeItemContentTypes,
  primaryContentCategory,
  selectVisibleItems,
  visibleItemsForFilter,
} from "./filter-logic.js";

const items = [
  { key: "hair", kind: "CASP", contentCategories: ["Hair", "Other CASP"] },
  { key: "facial-hair", kind: "CASP", contentCategories: ["Facial Hair", "Other CASP"] },
  { key: "accessory", kind: "CASP", contentCategories: ["Accessories", "Other CASP"] },
  { key: "clothes", kind: "CASP", contentCategories: ["Clothes", "Other CASP"] },
  { key: "shoes", kind: "CASP", contentCategories: ["Shoes", "Other CASP"] },
  { key: "skin", kind: "SkinTone", contentCategories: ["Skins"] },
  { key: "decor", kind: "OBJD", contentCategories: ["Decor", "Other Objects"] },
  { key: "slider", kind: "FACE", contentCategories: ["Sliders"] },
  { key: "pose-pack", kind: "Pose", contentCategories: ["Pose"] },
  { key: "clip", kind: "CLIP", contentCategories: ["Clip"] },
  { key: "fallback-clip", kind: "CLIP", contentCategories: [] },
  { key: "unclassified-casp", kind: "CASP", contentCategories: [] },
  { key: "fallback-objd", kind: "OBJD", contentCategories: [] },
  { key: "pattern", kind: "Pattern", contentCategories: [] },
];

assert.deepEqual(normalizeItemContentTypes(items[0]), ["Hair"]);
assert.deepEqual(normalizeItemContentTypes(items[1]), ["Facial Hair"]);
assert.deepEqual(normalizeItemContentTypes(items[2]), ["Accessories"]);
assert.deepEqual(normalizeItemContentTypes(items[3]), ["Clothes"]);
assert.deepEqual(normalizeItemContentTypes(items[4]), ["Shoes"]);
assert.deepEqual(normalizeItemContentTypes(items[5]), ["Skins"]);
assert.deepEqual(normalizeItemContentTypes(items[6]), ["Decor"]);
assert.deepEqual(normalizeItemContentTypes(items[7]), ["Sliders"]);
assert.deepEqual(normalizeItemContentTypes(items[8]), ["Pose"]);
assert.deepEqual(normalizeItemContentTypes(items[9]), ["Clip"]);
assert.deepEqual(normalizeItemContentTypes(items[10]), ["Clip"]);
assert.deepEqual(normalizeItemContentTypes(items[11]), ["Other CASP"]);
assert.deepEqual(normalizeItemContentTypes(items[12]), ["Other Objects"]);
assert.deepEqual(normalizeItemContentTypes(items[13]), []);

for (const item of items) {
  assert.equal(normalizeItemContentTypes(item).length <= 1, true);
}

assert.equal(primaryContentCategory(items[0]), "Hair");
assert.equal(itemMatchesContentFilter(items[0], "Other CASP"), false);
assert.equal(itemMatchesContentFilter(items[1], "Other CASP"), false);
assert.equal(itemMatchesContentFilter(items[2], "Other CASP"), false);
assert.deepEqual(visibleItemsForFilter(items, "Other CASP").map((item) => item.key), ["unclassified-casp"]);
assert.deepEqual(visibleItemsForFilter(items, "Other Objects").map((item) => item.key), ["fallback-objd"]);
assert.deepEqual(visibleItemsForFilter(items, "Pose").map((item) => item.key), ["pose-pack"]);
assert.deepEqual(visibleItemsForFilter(items, "Clip").map((item) => item.key), ["clip", "fallback-clip"]);

const counts = contentFilterCounts(items);
assert.equal(counts.all, 14);
assert.equal(counts.Hair, 1);
assert.equal(counts["Facial Hair"], 1);
assert.equal(counts.Accessories, 1);
assert.equal(counts["Other CASP"], 1);
assert.equal(counts.Decor, 1);
assert.equal(counts["Other Objects"], 1);
assert.equal(counts.Pose, 1);
assert.equal(counts.Clip, 2);

assert.deepEqual(availableContentFilters(items).map((filter) => filter.value), [
  "all",
  "Hair",
  "Facial Hair",
  "Clothes",
  "Shoes",
  "Accessories",
  "Other CASP",
  "Sliders",
  "Skins",
  "Pose",
  "Clip",
  "Decor",
  "Other Objects",
]);

assert.deepEqual(selectVisibleItems(items, ["skin"], "Other CASP"), ["skin", "unclassified-casp"]);
assert.deepEqual(
  deselectVisibleItems(items, ["skin", "unclassified-casp", "decor"], "Other CASP"),
  ["skin", "decor"],
);
assert.deepEqual(deselectVisibleItems(items, ["skin", "decor"], "all"), []);

// Regression: Select All / Select None receive the same already-filtered + searched
// subset rendered by app.js (getVisibleResources). They must not touch hidden matches.
const searchableItems = [
  { key: "red-hair", label: "Red Hair", kind: "CASP", contentCategories: ["Hair"] },
  { key: "blue-hair", label: "Blue Hair", kind: "CASP", contentCategories: ["Hair"] },
  { key: "red-shirt", label: "Red Shirt", kind: "CASP", contentCategories: ["Clothes"] },
  { key: "skin", label: "Default Skin", kind: "SkinTone", contentCategories: ["Skins"] },
];
const filterVisible = visibleItemsForFilter(searchableItems, "Hair");
const filterAndSearchVisible = filterVisible.filter((item) => item.label.toLowerCase().includes("red"));

assert.deepEqual(filterAndSearchVisible.map((item) => item.key), ["red-hair"]);
assert.deepEqual(selectVisibleItems(filterAndSearchVisible, ["skin"], "all"), ["skin", "red-hair"]);
assert.deepEqual(
  deselectVisibleItems(filterAndSearchVisible, ["red-hair", "blue-hair", "red-shirt", "skin"], "all"),
  ["blue-hair", "red-shirt", "skin"],
);
