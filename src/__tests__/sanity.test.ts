import { describe, it, expect } from 'vitest';

describe('frontend test infrastructure', () => {
  it('runs vitest with jsdom + jest-dom matchers', () => {
    expect(1 + 1).toBe(2);
    const el = document.createElement('div');
    el.textContent = 'hi';
    expect(el).toHaveTextContent('hi');
  });
});
