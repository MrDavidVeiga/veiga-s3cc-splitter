import assert from "node:assert/strict";
import {
  buildSameNameCounts,
  normalizeItemLabel,
  sameNameCountForItem,
  displayRelationForItem,
} from "./label-relations.js";

const items = [
  {
    key: "0x034AEECB-0x00000000-0x6625E7A598080A59",
    label: "1398476902:786335926",
    kind: "CASP",
  },
  {
    key: "0x034AEECB-0x00000000-0xDF40FA4BD874A5F5",
    label: "  1398476902:786335926  ",
    kind: "CASP",
  },
  {
    key: "0x034AEECB-0x00000000-0x1111111111111111",
    label: "Different CC",
    kind: "CASP",
  },
  {
    key: "0x034AEECB-0x00000000-0x2222222222222222",
    label: "",
    kind: "CASP",
  },
];

const counts = buildSameNameCounts(items);

assert.equal(normalizeItemLabel("  Hair Name  "), "hair name");
assert.equal(sameNameCountForItem(items[0], counts), 2);
assert.equal(sameNameCountForItem(items[1], counts), 2);
assert.equal(sameNameCountForItem(items[2], counts), 1);
assert.equal(sameNameCountForItem(items[3], counts), 0);

// Regression: two different resource keys with the same visible/internal name are
// only a same-name collision. This helper deliberately makes no duplicate-content
// claim from the label alone.
assert.notEqual(items[0].key, items[1].key);
assert.equal(normalizeItemLabel(items[0].label), normalizeItemLabel(items[1].label));

assert.deepEqual(
  displayRelationForItem({ relation: { tgiConflictCount: 2, duplicateCount: 2 } }, 4),
  { kind: "tgi-conflict", count: 2 },
);
assert.deepEqual(
  displayRelationForItem({ relation: { duplicateCount: 3, tgiConflictCount: 0 } }, 5),
  { kind: "duplicate", count: 3 },
);
assert.deepEqual(
  displayRelationForItem({ relation: { duplicateCount: 0, tgiConflictCount: 0 } }, 2),
  { kind: "same-name", count: 2 },
);
assert.equal(
  displayRelationForItem({ relation: { duplicateCount: 0, tgiConflictCount: 0 } }, 1),
  null,
);
