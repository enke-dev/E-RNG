import type { RouteConfig } from '@lit-labs/router';
import { Router } from '@lit-labs/router';
import type { ReactiveControllerHost } from 'lit';

import { currentRoute } from '../../state.js';

export function registerRouter(host: ReactiveControllerHost & HTMLElement, routes: RouteConfig[]) {
  return new Router(host, routes);
}

export function goto(path: string) {
  history.pushState({}, '', path);
  currentRoute.set(path);
  globalThis.dispatchEvent(new PopStateEvent('popstate'));
}
