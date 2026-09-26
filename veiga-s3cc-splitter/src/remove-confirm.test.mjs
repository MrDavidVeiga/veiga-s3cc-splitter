import assert from "node:assert/strict";
import { canUseReversibleMergeActions, formatRemoveConfirm } from "./remove-confirm.js";

const plan = (removed, total) => ({
  manifestPackagesToRemove: removed,
  manifestPackageTotal: total,
  resourcesToRemove: removed,
  selectedItems: removed,
  sharedResourcesPreserved: 0,
  selectedResourcesPreserved: 0,
});

const cases = [
  [1, 1],
  [1, 10],
  [2, 10],
  [10, 10],
];

for (const [removed, total] of cases) {
  const en = formatRemoveConfirm("en", plan(removed, total)).split("\n")[0];
  const pt = formatRemoveConfirm("pt", plan(removed, total)).split("\n")[0];
  const es = formatRemoveConfirm("es", plan(removed, total)).split("\n")[0];

  assert.equal(
    en,
    `This option will remove ${removed}/${total} ${removed === 1 ? "item" : "items"} from this merged package.`,
  );
  assert.equal(
    pt,
    `Esta opção irá remover ${removed}/${total} ${removed === 1 ? "conteúdo" : "conteúdos"} deste package mesclado.`,
  );
  assert.equal(
    es,
    `Esta opción eliminará ${removed}/${total} ${removed === 1 ? "contenido" : "contenidos"} de este package fusionado.`,
  );
}

assert.match(formatRemoveConfirm("es", plan(1, 10)), /fusionado/);
assert.doesNotMatch(formatRemoveConfirm("es", plan(1, 10)), /Fusion\b/);

assert.equal(canUseReversibleMergeActions(1), true);
assert.equal(canUseReversibleMergeActions(10), true);
assert.equal(canUseReversibleMergeActions(0), false);
assert.equal(canUseReversibleMergeActions(undefined), false);

const externalPlan = {
  manifestPackagesToRemove: 0,
  manifestPackageTotal: 0,
  resourcesToRemove: 3,
  selectedItems: 2,
  logicalItemTotal: 10,
  sharedResourcesPreserved: 1,
  selectedResourcesPreserved: 0,
};

for (const [language, expectedWarning, expectedFirstLine] of [
  ["en", /does not contain a Veiga Merge Manifest/, "This option will remove 2/10 items from this merged package."],
  ["pt", /não contém um Veiga Merge Manifest/, "Esta opção irá remover 2/10 conteúdos deste package mesclado."],
  ["es", /no contiene un Veiga Merge Manifest/, "Esta opción eliminará 2/10 contenidos de este package fusionado."],
]) {
  const message = formatRemoveConfirm(language, externalPlan);
  assert.match(message, expectedWarning);
  assert.equal(message.split("\n")[0], expectedFirstLine);
}

const externalFallback = formatRemoveConfirm("pt", {
  ...externalPlan,
  logicalItemTotal: 0,
  selectedItems: 1,
});
assert.equal(
  externalFallback.split("\n")[0],
  "Esta opção irá remover 1/1 conteúdo deste package mesclado.",
);


const singularPlan = {
  manifestPackagesToRemove: 1,
  manifestPackageTotal: 10,
  resourcesToRemove: 1,
  selectedItems: 1,
  sharedResourcesPreserved: 1,
  selectedResourcesPreserved: 1,
};

for (const language of ["en", "pt", "es"]) {
  assert.doesNotMatch(formatRemoveConfirm(language, singularPlan), /\(s\)|\(ns\)|\(es\)/);
}

assert.match(formatRemoveConfirm("en", singularPlan), /1 resource associated with 1 selected item\?/);
assert.match(formatRemoveConfirm("pt", singularPlan), /1 recurso associado a 1 item selecionado\?/);
assert.match(formatRemoveConfirm("es", singularPlan), /1 recurso asociado a 1 elemento seleccionado\?/);

const pluralPlan = {
  ...singularPlan,
  manifestPackagesToRemove: 2,
  resourcesToRemove: 2,
  selectedItems: 2,
  sharedResourcesPreserved: 2,
  selectedResourcesPreserved: 2,
};

assert.match(formatRemoveConfirm("en", pluralPlan), /2 resources associated with 2 selected items\?/);
assert.match(formatRemoveConfirm("pt", pluralPlan), /2 recursos associados a 2 itens selecionados\?/);
assert.match(formatRemoveConfirm("es", pluralPlan), /2 recursos asociados a 2 elementos seleccionados\?/);
