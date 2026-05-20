import "@testing-library/jest-dom";

// Polyfill ResizeObserver for tests (needed by ScrollArea component)
if (typeof global.ResizeObserver === "undefined") {
  global.ResizeObserver = class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}
