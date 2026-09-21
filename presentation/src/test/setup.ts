import "@testing-library/jest-dom/vitest";

// jsdom has no ResizeObserver — SlideCanvas uses one to rescale to its container.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;
