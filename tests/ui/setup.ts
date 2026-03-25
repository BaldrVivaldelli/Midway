import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

if (!window.matchMedia) {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn()
    }))
  });
}

if (!globalThis.ResizeObserver) {
  class ResizeObserverMock {
    observe() {}
    unobserve() {}
    disconnect() {}
  }

  // @ts-expect-error test shim
  globalThis.ResizeObserver = ResizeObserverMock;
}

if (!HTMLElement.prototype.scrollIntoView) {
  HTMLElement.prototype.scrollIntoView = vi.fn();
}


if (typeof Range !== "undefined") {
  if (!Range.prototype.getClientRects) {
    // @ts-expect-error test shim
    Range.prototype.getClientRects = () => ({
      length: 0,
      item: () => null,
      [Symbol.iterator]: function* () {}
    });
  }

  if (!Range.prototype.getBoundingClientRect) {
    // @ts-expect-error test shim
    Range.prototype.getBoundingClientRect = () => ({
      x: 0,
      y: 0,
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
      width: 0,
      height: 0,
      toJSON: () => ({})
    });
  }
}
