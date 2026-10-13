type DialogFocusOptions = {
  restoreFocus?: boolean;
  trigger?: HTMLElement | null;
};

const focusableSelector = 'button, a[href], input:not([type="hidden"]), select, textarea, [tabindex]';

function focusableElements(node: HTMLElement): HTMLElement[] {
  return [...node.querySelectorAll<HTMLElement>(focusableSelector)].filter((element) => {
    if (element.tabIndex < 0 || element.matches(':disabled') || element.closest('[hidden], [inert], [aria-hidden="true"]')) return false;
    for (let current: HTMLElement | null = element; current; current = current.parentElement) {
      const style = getComputedStyle(current);
      if (style.display === 'none' || style.visibility === 'hidden') return false;
      if (current === node) break;
    }
    return true;
  });
}

/** Keep keyboard focus inside a dialog and restore its trigger on close. */
export function focusDialog(node: HTMLElement, options: DialogFocusOptions = {}) {
  const trigger = options.trigger ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
  const previousTabIndex = node.getAttribute('tabindex');
  let active = true;
  node.tabIndex = -1;

  function focusInitial() {
    const elements = focusableElements(node);
    const target = elements.find((element) => element.hasAttribute('data-dialog-initial-focus')) ?? elements[0] ?? node;
    target.focus({ preventScroll: true });
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'Tab') return;
    const elements = focusableElements(node);
    const first = elements[0] ?? node;
    const last = elements.at(-1) ?? node;
    const focused = document.activeElement;
    if (!elements.length || !node.contains(focused) || focused === node || (event.shiftKey ? focused === first : focused === last)) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus({ preventScroll: true });
    }
  }

  function onFocusin(event: FocusEvent) {
    if (active && node.isConnected && event.target instanceof Node && !node.contains(event.target)) focusInitial();
  }

  node.addEventListener('keydown', onKeydown);
  document.addEventListener('focusin', onFocusin);
  queueMicrotask(() => {
    if (active && node.isConnected) focusInitial();
  });

  return {
    destroy() {
      active = false;
      node.removeEventListener('keydown', onKeydown);
      document.removeEventListener('focusin', onFocusin);
      if (previousTabIndex === null) node.removeAttribute('tabindex');
      else node.setAttribute('tabindex', previousTabIndex);
      if (options.restoreFocus !== false) queueMicrotask(() => {
        if (trigger?.isConnected && !trigger.closest('[inert]')) trigger.focus({ preventScroll: true });
      });
    },
  };
}
