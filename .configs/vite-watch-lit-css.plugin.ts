import type { ModuleNode, Plugin } from 'vite';

export interface LitCssWatchOptions {
  exclude?: string[];
  fullReload?: boolean;
}

export function litCssWatch({ exclude, fullReload = false }: LitCssWatchOptions = {}): Plugin {
  return {
    name: 'watch-lit-css',
    enforce: 'post',

    configureServer(server) {
      server.watcher.add(['src/**/*.css', ...(exclude ?? []).map(pattern => `!${pattern}`)]);

      if (fullReload) {
        server.watcher.on('change', () => {
          server.hot.send({ type: 'full-reload' });
        });
      }
    },

    ...(!fullReload
      ? {
          handleHotUpdate(ctx) {
            if (!ctx.file.endsWith('.css')) {
              return;
            }

            const modulesToUpdate = ctx.modules.reduce((acc, mod) => {
              ctx.server.moduleGraph.invalidateModule(mod);
              return new Set([
                ...acc,
                ...[...mod.importers].filter(importer => {
                  const relevant = /\.(js|ts|jsx|tsx)$/.test(importer.url);
                  if (relevant) {
                    ctx.server.moduleGraph.invalidateModule(importer);
                  }
                  return relevant;
                }),
              ]);
            }, new Set<ModuleNode>());

            if (modulesToUpdate.size > 0) {
              return Array.from(modulesToUpdate);
            }

            return ctx.modules;
          },
        }
      : {}),
  };
}
