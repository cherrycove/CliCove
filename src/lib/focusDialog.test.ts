// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { focusDialog } from './focusDialog';

const cleanups: Array<() => void> = [];
let trigger: HTMLButtonElement;

beforeEach(() => {
  document.body.innerHTML = '<button id="trigger">Open dialog</button><button id="outside">Outside</button>';
  trigger = document.querySelector<HTMLButtonElement>('#trigger')!;
  trigger.focus();
});
afterEach(() => {
  cleanups.splice(0).forEach((cleanup) => cleanup());
  document.body.innerHTML = '';
});

function dialog(contents: string, options?: { restoreFocus?: boolean; trigger?: HTMLElement }) {
  const node = document.createElement('div');
  node.innerHTML = contents;
  document.body.append(node);
  const action = focusDialog(node, options);
  cleanups.push(() => action.destroy());
  return { node, action };
}
function tab(shiftKey = false) {
  const event = new KeyboardEvent('keydown', { key: 'Tab', shiftKey, bubbles: true, cancelable: true });
  document.activeElement!.dispatchEvent(event);
  return event;
}

describe('dialog keyboard focus', () => {
  it('initially focuses the safe action instead of the first destructive action', async () => {
    const { node } = dialog('<button>Execute</button><button data-dialog-initial-focus>Cancel</button>');
    await Promise.resolve();
    expect(document.activeElement).toBe(node.lastElementChild);
  });

  it('wraps Tab in both directions while skipping hidden, disabled and inert controls', async () => {
    const { node } = dialog('<button id="first">First</button><button disabled>Disabled</button><div hidden><button>Hidden</button></div><div style="display:none"><button>CSS hidden</button></div><div inert><button>Inert</button></div><button tabindex="-1">List option</button><button id="last">Last</button>');
    await Promise.resolve();
    expect(tab(true).defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(node.querySelector('#last'));
    expect(tab().defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(node.querySelector('#first'));
  });

  it('respects controls that become disabled after the dialog opens', async () => {
    const { node } = dialog('<button id="first">Cancel</button><button id="last">Execute</button>');
    await Promise.resolve();
    node.querySelector<HTMLButtonElement>('#last')!.disabled = true;
    expect(tab().defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(node.querySelector('#first'));
  });

  it('prevents programmatic focus from escaping to the background', async () => {
    const { node } = dialog('<input aria-label="Search"><button tabindex="-1">Option</button>');
    await Promise.resolve();
    document.querySelector<HTMLButtonElement>('#outside')!.focus();
    expect(document.activeElement).toBe(node.querySelector('input'));
  });

  it('restores the trigger and releases focus containment after close', async () => {
    const { node, action } = dialog('<button>Cancel</button>');
    await Promise.resolve();
    action.destroy();
    node.remove();
    await Promise.resolve();
    expect(document.activeElement).toBe(trigger);
    document.querySelector<HTMLButtonElement>('#outside')!.focus();
    expect(document.activeElement?.id).toBe('outside');
  });

  it('allows the command palette to own its existing focus restoration', async () => {
    const { node, action } = dialog('<input aria-label="Search">', { restoreFocus: false });
    await Promise.resolve();
    action.destroy();
    node.remove();
    document.querySelector<HTMLButtonElement>('#outside')!.focus();
    await Promise.resolve();
    expect(document.activeElement?.id).toBe('outside');
  });

  it('restores an explicit pointer trigger even when the browser did not focus it', async () => {
    trigger.blur();
    const { node, action } = dialog('<button>Cancel</button>', { trigger });
    await Promise.resolve();
    action.destroy();
    node.remove();
    await Promise.resolve();
    expect(document.activeElement).toBe(trigger);
  });

  it('does not take focus if the dialog closes before its initial focus microtask', async () => {
    const { node, action } = dialog('<button>Cancel</button>', { restoreFocus: false });
    action.destroy();
    node.remove();
    await Promise.resolve();
    expect(document.activeElement).toBe(trigger);
  });

  it('keeps focus on the dialog when no control is available', async () => {
    const { node } = dialog('<button disabled>Waiting</button>');
    await Promise.resolve();
    expect(document.activeElement).toBe(node);
    expect(tab().defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(node);
  });
});
