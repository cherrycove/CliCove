// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import ScrollArea from './ScrollArea.svelte';

let observers: Array<{ callback: () => void; disconnect: ReturnType<typeof vi.fn> }>;
let frames: Map<number, FrameRequestCallback>;
let frameId = 0;

beforeEach(() => {
  observers = [];
  frames = new Map();
  vi.stubGlobal('ResizeObserver', class {
    disconnect = vi.fn();
    observe = vi.fn();
    constructor(public callback: () => void) { observers.push(this); }
  });
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { frames.set(++frameId, callback); return frameId; });
  vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id));
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

async function resize() {
  observers.forEach(observer => observer.callback());
  const callbacks = [...frames.values()];
  frames.clear();
  callbacks.forEach(callback => callback(0));
  await tick();
}

async function setup() {
  const result = render(ScrollArea, { label: '测试列表' });
  const viewport = result.container.querySelector<HTMLElement>('.scroll-viewport')!;
  const rail = result.container.querySelector<HTMLElement>('.scroll-rail')!;
  let contentHeight = 1000;
  Object.defineProperties(viewport, {
    clientHeight: { value: 200 }, scrollHeight: { get: () => contentHeight },
  });
  Object.defineProperty(rail, 'clientHeight', { value: 200 });
  rail.getBoundingClientRect = () => ({ top: 10 } as DOMRect);
  rail.setPointerCapture = vi.fn();
  rail.hasPointerCapture = vi.fn(() => true);
  rail.releasePointerCapture = vi.fn();
  await resize();
  return { ...result, viewport, rail, setContentHeight: (height: number) => { contentHeight = height; } };
}

function pointer(target: Element, type: string, clientY: number, extra: Record<string, unknown> = {}) {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.assign(event, { clientY, button: 0, pointerId: 1, isPrimary: true, ...extra });
  return fireEvent(target, event);
}

describe('scroll area interaction', () => {
  it('reflects native/programmatic scrolling and controls the same viewport by keyboard', async () => {
    const { viewport } = await setup();
    const thumb = screen.getByRole('scrollbar', { name: '测试列表滚动条' });
    expect(thumb.getAttribute('aria-controls')).toBe(viewport.id);
    viewport.scrollTop = 400;
    await fireEvent.scroll(viewport);
    expect(thumb.getAttribute('aria-valuenow')).toBe('50');
    await fireEvent.keyDown(thumb, { key: 'PageDown' });
    expect(viewport.scrollTop).toBe(570);
    await fireEvent.keyDown(thumb, { key: 'End' });
    expect(viewport.scrollTop).toBe(800);
    expect(thumb.getAttribute('aria-valuenow')).toBe('100');
    await fireEvent.keyDown(thumb, { key: 'Home' });
    await fireEvent.keyDown(thumb, { key: 'ArrowDown' });
    expect(viewport.scrollTop).toBe(44);
    await fireEvent.keyDown(thumb, { key: 'ArrowUp' });
    expect(viewport.scrollTop).toBe(0);
  });

  it('drags without jumping, clamps at both ends, and releases on cancellation', async () => {
    const { viewport, rail } = await setup();
    const thumb = screen.getByRole('scrollbar');
    await pointer(thumb, 'pointerdown', 20);
    expect(viewport.scrollTop).toBe(0);
    await pointer(rail, 'pointermove', 100);
    expect(viewport.scrollTop).toBe(400);
    await pointer(rail, 'pointermove', 500);
    expect(viewport.scrollTop).toBe(800);
    await pointer(rail, 'pointercancel', 500);
    await pointer(rail, 'pointermove', 20);
    expect(viewport.scrollTop).toBe(800);
    expect(rail.releasePointerCapture).toHaveBeenCalledWith(1);
    await pointer(rail, 'pointerdown', 0);
    expect(viewport.scrollTop).toBe(0);
    await pointer(rail, 'pointerup', 0);
  });

  it('jumps from the track and supports wheel line/page units without intercepting zoom', async () => {
    const { viewport, rail } = await setup();
    await pointer(rail, 'pointerdown', 110);
    expect(viewport.scrollTop).toBe(400);
    await pointer(rail, 'pointerup', 110);
    await fireEvent.wheel(rail, { deltaY: 2, deltaMode: 1 });
    expect(viewport.scrollTop).toBe(432);
    await fireEvent.wheel(rail, { deltaY: 1, deltaMode: 2 });
    expect(viewport.scrollTop).toBe(632);
    await fireEvent.wheel(rail, { deltaY: -10, ctrlKey: true });
    expect(viewport.scrollTop).toBe(632);
  });

  it('hides and restores the control as content changes and disconnects observers on removal', async () => {
    const { setContentHeight, unmount } = await setup();
    expect(screen.queryByRole('scrollbar')).not.toBeNull();
    setContentHeight(150);
    await resize();
    expect(screen.queryByRole('scrollbar')).toBeNull();
    expect(screen.getByRole('scrollbar', { hidden: true }).getAttribute('tabindex')).toBe('-1');
    setContentHeight(2000);
    await resize();
    expect(screen.getByRole('scrollbar').style.height).toBe('34px');
    await unmount();
    expect(observers[0].disconnect).toHaveBeenCalledOnce();
    expect(frames.size).toBe(0);
  });
});
