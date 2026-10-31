/**
 * Runs `task` at once, but never twice at the same time: calls while it runs make it run once more
 * when it is done, however many there were. For work that only matters for the latest state, like
 * a preview: each change starts it, a slow run does not pile up the ones behind it.
 */
export const latest = (task: () => Promise<void>) => {
  let running = false;
  let again = false;
  const run = async (): Promise<void> => {
    if (running) {
      again = true;
      return;
    }
    running = true;
    try {
      await task();
    } finally {
      running = false;
      if (again) {
        again = false;
        void run();
      }
    }
  };
  return run;
};
