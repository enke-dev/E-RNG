import { registerIconLibrary } from '@awesome.me/webawesome/dist/components/icon/library.js';
import check from '@fortawesome/fontawesome-free/svgs/solid/check.svg?url';
import info from '@fortawesome/fontawesome-free/svgs/solid/circle-info.svg?url';
import save from '@fortawesome/fontawesome-free/svgs/solid/floppy-disk.svg?url';
import link from '@fortawesome/fontawesome-free/svgs/solid/link.svg?url';
import unlink from '@fortawesome/fontawesome-free/svgs/solid/link-slash.svg?url';
import trash from '@fortawesome/fontawesome-free/svgs/solid/trash.svg?url';
import warning from '@fortawesome/fontawesome-free/svgs/solid/triangle-exclamation.svg?url';

/**
 * The icons of the app, shipped with it: the default library of Web Awesome loads its icons from
 * a CDN, which an app that works offline cannot do. Names say what an icon means, not how it
 * looks, so one place decides which picture stands for "save".
 */
const ICONS: Record<string, string> = {
  check,
  info,
  link,
  save,
  trash,
  unlink,
  warning,
};

export type IconName = keyof typeof ICONS;

/** Makes `<wa-icon name="save">` work, call it once before anything renders */
export function registerIcons() {
  registerIconLibrary('default', {
    resolver: name => ICONS[name] ?? '',
    mutator: svg => svg.setAttribute('fill', 'currentColor'),
  });
}
