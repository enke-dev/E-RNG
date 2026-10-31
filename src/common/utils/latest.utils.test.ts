import { describe, expect, it } from 'vitest';

import { latest } from './latest.utils.js';

/** A task that waits until it is let go, counting its runs */
const gated = () => {
  const releases: (() => void)[] = [];
  let runs = 0;
  const task = () => {
    runs++;
    return new Promise<void>(resolve => releases.push(resolve));
  };
  const release = async () => {
    releases.shift()?.();
    // let the finally of the run and the next run start
    await new Promise(resolve => setTimeout(resolve));
  };
  return { task, release, runs: () => runs };
};

describe('latest', () => {
  it('runs at once', () => {
    const { task, runs } = gated();
    void latest(task)();
    expect(runs()).toBe(1);
  });

  it('runs once more after calls that came while it ran, however many', async () => {
    const { task, release, runs } = gated();
    const run = latest(task);
    void run();
    void run();
    void run();
    void run();
    expect(runs()).toBe(1);
    await release();
    expect(runs()).toBe(2);
    await release();
    expect(runs()).toBe(2);
  });

  it('runs again after a failure', async () => {
    let runs = 0;
    const run = latest(() => {
      runs++;
      return Promise.reject(new Error('broken'));
    });
    await run().catch(() => undefined);
    await run().catch(() => undefined);
    expect(runs).toBe(2);
  });
});
