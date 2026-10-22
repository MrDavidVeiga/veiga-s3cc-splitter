function count(value) {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}

function form(value, singular, plural) {
  return count(value) === 1 ? singular : plural;
}

export function canUseReversibleMergeActions(manifestPackageCount) {
  return count(manifestPackageCount) > 0;
}

export function formatRemoveConfirm(language, plan = {}) {
  const removed = count(plan.manifestPackagesToRemove);
  const total = count(plan.manifestPackageTotal);
  const resourcesToRemove = count(plan.resourcesToRemove);
  const selectedItems = count(plan.selectedItems);
  const logicalItemTotal = count(plan.logicalItemTotal);
  const sharedResourcesPreserved = count(plan.sharedResourcesPreserved);
  const selectedResourcesPreserved = count(plan.selectedResourcesPreserved);
  const manifestBacked = total > 0;

  if (!manifestBacked) {
    const itemTotal = logicalItemTotal || selectedItems;

    if (language === "pt") {
      const contentLabel = selectedItems === 1 ? "conteúdo" : "conteúdos";
      return `Esta opção irá remover ${selectedItems}/${itemTotal} ${contentLabel} deste package mesclado.\n\nEste package não contém um Veiga Merge Manifest, portanto esta operação não poderá ser revertida automaticamente.\n\n${resourcesToRemove} ${form(resourcesToRemove, "recurso associado será removido", "recursos associados serão removidos")} quando for seguro fazê-lo. ${sharedResourcesPreserved} ${form(sharedResourcesPreserved, "recurso compartilhado detectado será preservado", "recursos compartilhados detectados serão preservados")}.\nUm backup bit a bit do original será criado e verificado antes da substituição final.`;
    }

    if (language === "es") {
      const contentLabel = selectedItems === 1 ? "contenido" : "contenidos";
      return `Esta opción eliminará ${selectedItems}/${itemTotal} ${contentLabel} de este package fusionado.\n\nEste package no contiene un Veiga Merge Manifest, por lo que esta operación no podrá revertirse automáticamente.\n\nSe ${form(resourcesToRemove, "eliminará", "eliminarán")} ${resourcesToRemove} ${form(resourcesToRemove, "recurso asociado", "recursos asociados")} cuando sea seguro hacerlo. Se ${form(sharedResourcesPreserved, "conservará", "conservarán")} ${sharedResourcesPreserved} ${form(sharedResourcesPreserved, "recurso compartido detectado", "recursos compartidos detectados")}.\nSe creará y verificará una copia de seguridad bit a bit del original antes del reemplazo final.`;
    }

    const itemLabel = selectedItems === 1 ? "item" : "items";
    return `This option will remove ${selectedItems}/${itemTotal} ${itemLabel} from this merged package.\n\nThis package does not contain a Veiga Merge Manifest, so this operation cannot be automatically reversed.\n\n${resourcesToRemove} ${form(resourcesToRemove, "associated resource", "associated resources")} will be removed when it is safe to do so. ${sharedResourcesPreserved} ${form(sharedResourcesPreserved, "detected shared resource", "detected shared resources")} will be preserved.\nA bit-for-bit backup of the original will be created and verified before the final replacement.`;
  }

  if (language === "pt") {
    const contentLabel = removed === 1 ? "conteúdo" : "conteúdos";
    return `Esta opção irá remover ${removed}/${total} ${contentLabel} deste package mesclado.\n\nRemover ${resourcesToRemove} ${form(resourcesToRemove, "recurso associado", "recursos associados")} a ${selectedItems} ${form(selectedItems, "item selecionado", "itens selecionados")}?\n${sharedResourcesPreserved} ${form(sharedResourcesPreserved, "recurso compartilhado será preservado", "recursos compartilhados serão preservados")}.${selectedResourcesPreserved ? `\n${selectedResourcesPreserved} ${form(selectedResourcesPreserved, "recurso selecionado é compartilhado e permanecerá", "recursos selecionados são compartilhados e permanecerão")} no package mesclado.` : ""}\nUm backup bit a bit do original será criado antes da substituição final.`;
  }

  if (language === "es") {
    const contentLabel = removed === 1 ? "contenido" : "contenidos";
    return `Esta opción eliminará ${removed}/${total} ${contentLabel} de este package fusionado.\n\n¿Eliminar ${resourcesToRemove} ${form(resourcesToRemove, "recurso asociado", "recursos asociados")} a ${selectedItems} ${form(selectedItems, "elemento seleccionado", "elementos seleccionados")}?\nSe ${form(sharedResourcesPreserved, "conservará", "conservarán")} ${sharedResourcesPreserved} ${form(sharedResourcesPreserved, "recurso compartido", "recursos compartidos")}.${selectedResourcesPreserved ? `\n${selectedResourcesPreserved} ${form(selectedResourcesPreserved, "recurso seleccionado es compartido y permanecerá", "recursos seleccionados son compartidos y permanecerán")} en el package fusionado.` : ""}\nSe creará una copia de seguridad bit a bit del original antes del reemplazo final.`;
  }

  const itemLabel = removed === 1 ? "item" : "items";
  return `This option will remove ${removed}/${total} ${itemLabel} from this merged package.\n\nRemove ${resourcesToRemove} ${form(resourcesToRemove, "resource", "resources")} associated with ${selectedItems} selected ${form(selectedItems, "item", "items")}?\n${sharedResourcesPreserved} shared ${form(sharedResourcesPreserved, "resource", "resources")} will be preserved.${selectedResourcesPreserved ? `\n${selectedResourcesPreserved} selected ${form(selectedResourcesPreserved, "resource is", "resources are")} shared and will remain in the merged package.` : ""}\nA bit-for-bit backup of the original will be created before the final replacement.`;
}
