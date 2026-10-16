// formats the whole repo once instead of per file; lint-staged re-stages what changed
export default { '*': () => 'bun run format' };
