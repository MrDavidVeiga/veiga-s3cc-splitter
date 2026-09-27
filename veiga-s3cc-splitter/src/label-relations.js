export function normalizeItemLabel(label) {
  return String(label || "").trim().toLowerCase();
}

export function buildSameNameCounts(items) {
  const counts = new Map();

  for (const item of items || []) {
    const key = normalizeItemLabel(item?.label);
    if (!key) continue;
    counts.set(key, (counts.get(key) || 0) + 1);
  }

  return counts;
}

export function sameNameCountForItem(item, counts) {
  const key = normalizeItemLabel(item?.label);
  if (!key) return 0;
  return counts?.get(key) || 0;
}

export function displayRelationForItem(item, sameNameCount) {
  const duplicateCount = Number(item?.relation?.duplicateCount || 0);
  const tgiConflictCount = Number(item?.relation?.tgiConflictCount || 0);

  if (tgiConflictCount > 1) {
    return { kind: "tgi-conflict", count: tgiConflictCount };
  }
  if (duplicateCount > 1) {
    return { kind: "duplicate", count: duplicateCount };
  }
  if (Number(sameNameCount) > 1) {
    return { kind: "same-name", count: Number(sameNameCount) };
  }

  return null;
}
