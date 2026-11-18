import assert from "node:assert/strict";
import { createModalController, runConfirmedAction } from "./modal.js";

class FakeClassList {
  constructor(...initial) {
    this.values = new Set(initial);
  }
  add(value) { this.values.add(value); }
  remove(value) { this.values.delete(value); }
  contains(value) { return this.values.has(value); }
  toggle(value, force) {
    if (force === undefined) force = !this.contains(value);
    if (force) this.add(value); else this.remove(value);
    return force;
  }
}

class FakeElement {
  constructor(doc, classes = []) {
    this.doc = doc;
    this.classList = new FakeClassList(...classes);
    this.listeners = new Map();
    this.attributes = new Map();
    this.textContent = "";
    this.children = [];
    this.tagName = null;
    this.disabled = false;
    this.focusCount = 0;
  }
  replaceChildren(...nodes) {
    this.children = nodes;
    this.textContent = nodes.map((node) => node.textContent ?? "").join("");
  }
  addEventListener(type, handler) {
    if (!this.listeners.has(type)) this.listeners.set(type, []);
    this.listeners.get(type).push(handler);
  }
  dispatch(type, event = {}) {
    for (const handler of this.listeners.get(type) || []) handler(event);
  }
  click() { this.dispatch("click", { target: this }); }
  focus() {
    this.doc.activeElement = this;
    this.focusCount += 1;
  }
  setAttribute(name, value) { this.attributes.set(name, String(value)); }
  getAttribute(name) { return this.attributes.get(name); }
}

function fixture() {
  const doc = { activeElement: null };
  doc.createElement = (tag) => {
    const element = new FakeElement(doc);
    element.tagName = tag;
    return element;
  };
  doc.createTextNode = (value) => ({ textContent: String(value), nodeType: 3 });
  const trigger = new FakeElement(doc);
  trigger.focus();
  const root = new FakeElement(doc, ["hidden"]);
  const panel = new FakeElement(doc);
  const title = new FakeElement(doc);
  const message = new FakeElement(doc);
  const closeButton = new FakeElement(doc, ["app-modal-close"]);
  const cancelButton = new FakeElement(doc, ["secondary-btn", "hidden"]);
  const primaryButton = new FakeElement(doc, ["primary-btn"]);
  const modal = createModalController(
    { root, panel, title, message, closeButton, cancelButton, primaryButton },
    { document: doc, getCloseLabel: () => "Close" },
  );
  return { doc, trigger, root, title, message, closeButton, cancelButton, primaryButton, modal };
}

// A) information modal opens and closes.
{
  const f = fixture();
  const pending = f.modal.showInfo({ titleText: "Info", messageText: "Ready", primaryLabel: "OK" });
  assert.equal(f.root.classList.contains("hidden"), false);
  assert.equal(f.root.getAttribute("aria-hidden"), "false");
  assert.equal(f.title.textContent, "Info");
  assert.equal(f.message.textContent, "Ready");
  assert.equal(f.doc.activeElement, f.primaryButton);
  f.primaryButton.click();
  assert.equal(await pending, true);
  assert.equal(f.root.classList.contains("hidden"), true);
  assert.equal(f.doc.activeElement, f.trigger);
}

// B) error modal opens and closes.
{
  const f = fixture();
  const pending = f.modal.showError({ titleText: "Error", messageText: "Failure", primaryLabel: "Close" });
  assert.equal(f.modal.activeKind(), "error");
  f.primaryButton.click();
  assert.equal(await pending, true);
}

// C) confirm returns true when primary action is chosen.
{
  const f = fixture();
  const pending = f.modal.showConfirm({
    titleText: "Remove",
    messageText: "2/10",
    cancelLabel: "Cancel",
    primaryLabel: "Remove",
    danger: true,
  });
  assert.equal(f.doc.activeElement, f.cancelButton);
  assert.equal(f.primaryButton.classList.contains("danger-btn"), true);
  f.primaryButton.click();
  assert.equal(await pending, true);
}

// D) confirm returns false when cancelled.
{
  const f = fixture();
  const pending = f.modal.showConfirm({
    titleText: "Remove",
    messageText: "2/10",
    cancelLabel: "Cancel",
    primaryLabel: "Remove",
  });
  f.cancelButton.click();
  assert.equal(await pending, false);
}

// E) Escape cancels confirmation and does not activate the dangerous action.
{
  const f = fixture();
  const pending = f.modal.showConfirm({
    titleText: "Remove",
    messageText: "2/10",
    cancelLabel: "Cancel",
    primaryLabel: "Remove",
  });
  let prevented = false;
  f.root.dispatch("keydown", { key: "Escape", preventDefault() { prevented = true; } });
  assert.equal(await pending, false);
  assert.equal(prevented, true);
}

// F/G) the destructive operation only runs after a positive confirmation.
{
  let mutations = 0;
  const positive = await runConfirmedAction(async () => true, async () => {
    mutations += 1;
    return "removed";
  });
  assert.deepEqual(positive, { confirmed: true, result: "removed" });
  assert.equal(mutations, 1);

  const cancelled = await runConfirmedAction(async () => false, async () => {
    mutations += 1;
    return "should-not-run";
  });
  assert.deepEqual(cancelled, { confirmed: false, result: undefined });
  assert.equal(mutations, 1);
}

// H) multiline X/X copy can be shown without modification inside the modal.
{
  const f = fixture();
  const copy = "Esta opção irá remover 2/10 conteúdos deste package mesclado.\n\n2 recurso(s) serão removidos.";
  const pending = f.modal.showConfirm({
    titleText: "Remover do Mesclado",
    messageText: copy,
    cancelLabel: "Cancelar",
    primaryLabel: "Remover",
  });
  assert.equal(f.message.textContent, copy);
  f.cancelButton.click();
  await pending;
}

// H2) The removal backup is bold, separately spaced, and stays literal text.
{
  const f = fixture();
  const copy = '2 associated resources will be removed. 1 detected shared resource will be preserved.\n\nA bit-for-bit backup of the original will be created and verified before the final replacement.';
  const pending = f.modal.showConfirm({
    titleText: "Remove",
    messageText: copy,
    emphasizeLastParagraph: true,
    cancelLabel: "Cancel",
    primaryLabel: "Remove",
  });
  assert.equal(f.message.textContent, copy);
  assert.equal(f.message.children.length, 2);
  assert.equal(f.message.children[0].nodeType, 3);
  assert.equal(f.message.children[0].textContent.endsWith("\n\n"), true);
  assert.equal(f.message.children[1].tagName, "strong");
  assert.equal(f.message.children[1].className, "app-modal-emphasis");
  assert.equal(f.message.children[1].textContent,
    "A bit-for-bit backup of the original will be created and verified before the final replacement.");
  f.cancelButton.click();
  await pending;
}

// H3) Even markup-like resource names are escaped and never parsed as HTML.
{
  const f = fixture();
  const copy = '<img src=x onerror=alert(1)>\n\nDo not lose the original.';
  const pending = f.modal.showConfirm({
    titleText: "Warning",
    messageText: copy,
    emphasizeLastParagraph: true,
    cancelLabel: "Cancel",
    primaryLabel: "Proceed",
  });
  assert.equal(f.message.textContent, copy);
  assert.equal(f.message.children[0].textContent.includes("<img"), true);
  assert.equal(f.message.children[1].textContent, "Do not lose the original.");
  f.cancelButton.click();
  await pending;
}

// I) close X dismisses a normal modal and restores focus.
{
  const f = fixture();
  const pending = f.modal.showInfo({ titleText: "Info", messageText: "Ready", primaryLabel: "OK" });
  assert.equal(f.closeButton.getAttribute("aria-label"), "Close");
  f.closeButton.click();
  assert.equal(await pending, true);
  assert.equal(f.doc.activeElement, f.trigger);
}

// J) close X on confirmation is equivalent to Cancel and never runs the action.
{
  const f = fixture();
  let mutations = 0;
  const result = await runConfirmedAction(
    () => {
      const pending = f.modal.showConfirm({
        titleText: "Remove",
        messageText: "2/10",
        cancelLabel: "Cancel",
        primaryLabel: "Remove",
        danger: true,
      });
      f.closeButton.click();
      return pending;
    },
    async () => {
      mutations += 1;
    },
  );
  assert.deepEqual(result, { confirmed: false, result: undefined });
  assert.equal(mutations, 0);
}

// K) success modal exposes the success kind for semantic styling.
{
  const f = fixture();
  const pending = f.modal.showSuccess({ titleText: "Success", messageText: "Done", primaryLabel: "OK" });
  assert.equal(f.modal.activeKind(), "success");
  f.primaryButton.click();
  assert.equal(await pending, true);
}
