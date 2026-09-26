function setHidden(element, hidden) {
  element?.classList.toggle("hidden", hidden);
}

function focusableButtons(cancelButton, primaryButton) {
  return [cancelButton, primaryButton].filter(
    (button) => button && !button.classList.contains("hidden") && !button.disabled,
  );
}

export function createModalController(elements, environment = {}) {
  const {
    root,
    panel,
    title,
    message,
    closeButton,
    cancelButton,
    primaryButton,
  } = elements;
  const doc = environment.document || globalThis.document;
  const getCloseLabel = environment.getCloseLabel || (() => "Close");

  let activeResolve = null;
  let previousFocus = null;
  let activeKind = null;

  function finish(result) {
    if (!activeResolve) return;
    const resolve = activeResolve;
    activeResolve = null;
    activeKind = null;
    setHidden(root, true);
    root?.setAttribute("aria-hidden", "true");
    if (previousFocus && typeof previousFocus.focus === "function") {
      previousFocus.focus();
    }
    previousFocus = null;
    resolve(result);
  }

  function showModal({
    kind = "info",
    titleText = "",
    messageText = "",
    primaryLabel = "OK",
    cancelLabel = "",
    closeLabel = getCloseLabel(),
    danger = false,
  }) {
    if (activeResolve) finish(false);

    previousFocus = doc?.activeElement || null;
    activeKind = kind;

    if (title) title.textContent = titleText;
    if (message) message.textContent = messageText;
    if (closeButton) {
      closeButton.setAttribute("aria-label", closeLabel);
      closeButton.setAttribute("title", closeLabel);
    }

    if (primaryButton) {
      primaryButton.textContent = primaryLabel;
      primaryButton.classList.toggle("danger-btn", danger);
      primaryButton.classList.toggle("primary-btn", !danger);
    }

    const isConfirm = kind === "confirm";
    if (cancelButton) {
      cancelButton.textContent = cancelLabel;
      setHidden(cancelButton, !isConfirm);
    }

    root?.setAttribute("data-modal-kind", kind);
    root?.setAttribute("aria-hidden", "false");
    setHidden(root, false);

    const target = isConfirm ? cancelButton : primaryButton;
    if (target && typeof target.focus === "function") target.focus();
    else if (panel && typeof panel.focus === "function") panel.focus();

    return new Promise((resolve) => {
      activeResolve = resolve;
    });
  }

  primaryButton?.addEventListener("click", () => finish(true));
  cancelButton?.addEventListener("click", () => finish(false));
  closeButton?.addEventListener("click", () => finish(activeKind === "confirm" ? false : true));

  root?.addEventListener("keydown", (event) => {
    if (!activeResolve) return;

    if (event.key === "Escape") {
      event.preventDefault?.();
      finish(activeKind === "confirm" ? false : true);
      return;
    }

    if (event.key !== "Tab") return;
    const buttons = [closeButton, ...focusableButtons(cancelButton, primaryButton)].filter(
      (button) => button && !button.disabled,
    );
    if (!buttons.length) return;

    const currentIndex = buttons.indexOf(doc?.activeElement);
    const direction = event.shiftKey ? -1 : 1;
    const nextIndex =
      currentIndex < 0
        ? 0
        : (currentIndex + direction + buttons.length) % buttons.length;
    event.preventDefault?.();
    buttons[nextIndex].focus();
  });

  return {
    showModal,
    showInfo(options) {
      return showModal({ ...options, kind: "info" });
    },
    showError(options) {
      return showModal({ ...options, kind: "error" });
    },
    showSuccess(options) {
      return showModal({ ...options, kind: "success" });
    },
    showWarning(options) {
      return showModal({ ...options, kind: "warning" });
    },
    showConfirm(options) {
      return showModal({ ...options, kind: "confirm" });
    },
    close() {
      finish(false);
    },
    isOpen() {
      return Boolean(activeResolve);
    },
    activeKind() {
      return activeKind;
    },
  };
}

export async function runConfirmedAction(confirmAction, action) {
  const confirmed = await confirmAction();
  if (!confirmed) return { confirmed: false, result: undefined };
  return { confirmed: true, result: await action() };
}
