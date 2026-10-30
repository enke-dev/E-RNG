# E-RNG

# [0.7.0](https://github.com/enke-dev/E-RNG/compare/v0.6.3...v0.7.0) (2026-10-30)

### Bug Fixes

* **settings:** keep part opened from the sample when its tab group first shows ([53eb869](https://github.com/enke-dev/E-RNG/commit/53eb8695383018a8f0a5a64b43c5fad7d95a9719))

### Features

* **settings:** close the open part by clicking it in the sample again ([08eb719](https://github.com/enke-dev/E-RNG/commit/08eb719926f2230a289b6d6a491b83c6a68f9a32))
* **settings:** link layout parts in form and sample with a curve ([42b5855](https://github.com/enke-dev/E-RNG/commit/42b58559fd380e52347b30808678b3e136fb243f))

## [0.6.3](https://github.com/enke-dev/E-RNG/compare/v0.6.2...v0.6.3) (2026-10-30)

### Performance Improvements

* release builds use one codegen unit and no symbols ([d9b2435](https://github.com/enke-dev/E-RNG/commit/d9b24353fa58e0efe6242b1e2f6fe7e6347cea90))
* the mustang jar leaves out what the app never runs ([e9f7606](https://github.com/enke-dev/E-RNG/commit/e9f7606042b3af8cda6561cff1c2e3fe064fa833))
* typst is compiled into the app, no sidecar ([739c0ab](https://github.com/enke-dev/E-RNG/commit/739c0abce74b34d44aa550be217e79d2dc472ce3))

## [0.6.2](https://github.com/enke-dev/E-RNG/compare/v0.6.1...v0.6.2) (2026-10-29)

### Bug Fixes

* an update installs when the user says so ([bcc5c4b](https://github.com/enke-dev/E-RNG/commit/bcc5c4b8e96bbfaf266fd7146ef3f8138714dc97))

## [0.6.1](https://github.com/enke-dev/E-RNG/compare/v0.6.0...v0.6.1) (2026-10-29)

### Bug Fixes

* no space between a number and its unit in the app ([1ed81f0](https://github.com/enke-dev/E-RNG/commit/1ed81f014a6935147cc5fd4e942893dd1fcc5ce5))
* vat rows list the highest rate first; an interrupted restore is shown ([f74b875](https://github.com/enke-dev/E-RNG/commit/f74b8753fa53a9d0ec57ed9fb95973cc2f828193))

# [0.6.0](https://github.com/enke-dev/E-RNG/compare/v0.5.2...v0.6.0) (2026-10-29)

### Bug Fixes

* a customer without an address leaves no empty elements in the XML ([36633c7](https://github.com/enke-dev/E-RNG/commit/36633c79e0edb03a7d9dc3b27c00a88f448214d4))
* fonts with Mac-only names and font collections import fully ([2c5b1ab](https://github.com/enke-dev/E-RNG/commit/2c5b1abc3d5b8dbcacc4c3262a6a257273393123))
* layouts are tagged integriert or individuell, their parts are Elemente ([ec48bad](https://github.com/enke-dev/E-RNG/commit/ec48badf3493e89f1061802f8deac44507a4b446))
* part boxes are read with typst eval, query is deprecated ([f849ecc](https://github.com/enke-dev/E-RNG/commit/f849ecc0cecaa1eb812bb7e31b1f7b8746014865))
* the layout editor keeps its place when saving and switching pages ([4ab4255](https://github.com/enke-dev/E-RNG/commit/4ab4255ff3f646f08b65399435a41025e22cd4f6))

### Features

* a customer address is required above 250 euros gross ([0a01e72](https://github.com/enke-dev/E-RNG/commit/0a01e72c6ec103a420c7cc77ccbbf4dad7eb515e))
* a debug build can start from another data folder ([83b195e](https://github.com/enke-dev/E-RNG/commit/83b195e7aa5c4d617402990c9a76a8fe9fee16cb))
* an invoice can name its offer ([ff73fae](https://github.com/enke-dev/E-RNG/commit/ff73faeb347da1ade4ce019e4c1560185c4f5045))
* back up and restore all data ([4845e24](https://github.com/enke-dev/E-RNG/commit/4845e24945ca89188f10eb475e15423a73935708))
* drafts read as Rechnungsentwurf and export as PDF ([243e3a0](https://github.com/enke-dev/E-RNG/commit/243e3a02dcd50877be85a8b5d38db5f1ba94e212))
* drafts save themselves ([8202d13](https://github.com/enke-dev/E-RNG/commit/8202d13d24d7200be7b67c92917c9a0ebed3ded3))
* invoice list filters by status and shows archived invoices ([b9f1247](https://github.com/enke-dev/E-RNG/commit/b9f1247d739e7da92f81be5fd8818c0000f32ea9))
* layout lines, texts and other parts take a color ([902df2f](https://github.com/enke-dev/E-RNG/commit/902df2f85af84683f44652e2d6e69e753dd05728)), closes [#rrggbb](https://github.com/enke-dev/E-RNG/issues/rrggbb)
* layout parts can be reordered ([b5e147f](https://github.com/enke-dev/E-RNG/commit/b5e147fcec4126bd5f959659354f1b3a4be1374d))
* layouts fill single seller fields; sellers get mobile, fax, website and bank name ([280e10f](https://github.com/enke-dev/E-RNG/commit/280e10fad2be44adcb7aa5991b4213a7868bd842))
* number sequences continue from a start number and by fiscal year ([87ed0f1](https://github.com/enke-dev/E-RNG/commit/87ed0f1c294c9aeeef05739d3f3a5b22a8262295))
* offers and invoices are linked, an offer becomes an invoice draft ([3bc16cf](https://github.com/enke-dev/E-RNG/commit/3bc16cfb03040384eededafef9e51547cececdc9))
* offers as a second kind of document ([8e895e0](https://github.com/enke-dev/E-RNG/commit/8e895e0ad1a67d48059cb30a10249bb10891091e))
* offers get their own list, editor and settings ([0912b99](https://github.com/enke-dev/E-RNG/commit/0912b991d05172862a77e71c5279ae46a1eaf9ad))
* service as a date or a period ([b75dc6d](https://github.com/enke-dev/E-RNG/commit/b75dc6d2c31b643f8c2367afeb8994a7b003863a))
* the layout preview shows invisible characters with its guides ([06cf9b6](https://github.com/enke-dev/E-RNG/commit/06cf9b670d57deb4ddaf8b8a5db4e897f6448a5b))
* the page number can stay away from single pages ([b0fa3fd](https://github.com/enke-dev/E-RNG/commit/b0fa3fd6a85d4db6b51c0eb7b6f8d747a65bad0d))
* the positions show the VAT rate in a column of its own ([bcf4a55](https://github.com/enke-dev/E-RNG/commit/bcf4a55a4d9bb0bac111a0d26a2ef0d656f8b7a9))
* units are configured per context ([41a9d7f](https://github.com/enke-dev/E-RNG/commit/41a9d7f940d62af5e0930d3675ca96cdd27ff996))
* **updater:** spin in a square only while nothing can be counted ([5a439c8](https://github.com/enke-dev/E-RNG/commit/5a439c8dd46303a3c656bd4eedac99fe01cbac77))
* vat rates per context and position ([e1f7877](https://github.com/enke-dev/E-RNG/commit/e1f7877e38ca1a8928e34a12d96522a7dd28f6fa))

## [0.5.2](https://github.com/enke-dev/E-RNG/compare/v0.5.1...v0.5.2) (2026-10-26)

### Bug Fixes

* search of customers and positions matches the context names ([5cf9a5d](https://github.com/enke-dev/E-RNG/commit/5cf9a5ddb2ee1c6d8701458acf6f572b018f5109))

## [0.5.1](https://github.com/enke-dev/E-RNG/compare/v0.5.0...v0.5.1) (2026-10-26)

### Bug Fixes

* add missing Einstellungen in the menu bar ([26fdeba](https://github.com/enke-dev/E-RNG/commit/26fdebab4fe616d7f4d445a65e8b0997059fc44e))
* fonts stored before their style was read get it from the file ([5025453](https://github.com/enke-dev/E-RNG/commit/5025453132186164d755ed090ee7691c5932f289))

# [0.5.0](https://github.com/enke-dev/E-RNG/compare/v0.4.0...v0.5.0) (2026-10-24)

### Features

* create invoices in one step and keep their originals ([ddd7f9f](https://github.com/enke-dev/E-RNG/commit/ddd7f9faeb9c24782727fc4b1fe869e6ce13289a))
* update progress in the button and a menu entry to look for updates ([771765e](https://github.com/enke-dev/E-RNG/commit/771765ecb08cfa0bce1b5eff51665db01f302979))

# [0.4.0](https://github.com/enke-dev/E-RNG/compare/v0.3.1...v0.4.0) (2026-10-20)

### Bug Fixes

* the divider of a split panel stays behind the page footer ([4d3977b](https://github.com/enke-dev/E-RNG/commit/4d3977b9b0d75aa7fea9fb78b8fac7c44d8182e8))

### Features

* a tax number alone identifies the seller ([d640f71](https://github.com/enke-dev/E-RNG/commit/d640f7156e3f4da07b26a03f677a67bb6c76a2bc))
* contexts for several issuers ([ebb4ba3](https://github.com/enke-dev/E-RNG/commit/ebb4ba39f31a81cac4edc69f053e2a6197b9ee44))
* guides show the printable area and the punch holes, and are remembered ([1362868](https://github.com/enke-dev/E-RNG/commit/136286828a195f4eeb79e31623d52a57c18c6ed4))
* invoice numbers follow a pattern, legal information is one field ([18fa96b](https://github.com/enke-dev/E-RNG/commit/18fa96bce183346ab70e53b5ab4e52464533583f))
* invoices without VAT and without payment term ([ed56c8a](https://github.com/enke-dev/E-RNG/commit/ed56c8a7acbcb2080d1e6e1dd1c857929a018ea9))
* layouts can be exported to and imported from a file ([c5bb1e8](https://github.com/enke-dev/E-RNG/commit/c5bb1e8634c453fa7f5152cde7e510ec76308fa9))
* let the user move the divider and restore the window at start ([b4d241f](https://github.com/enke-dev/E-RNG/commit/b4d241fb9ecc798a3a83aeba468bb54cbf0bb7d7))
* tell the hovered part from the selected one ([1df7f9c](https://github.com/enke-dev/E-RNG/commit/1df7f9cb940b58d4b88775313d3a73dd85c7df48))

## [0.3.1](https://github.com/enke-dev/E-RNG/compare/v0.3.0...v0.3.1) (2026-10-19)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

## [0.6.3](https://github.com/enke-dev/E-RNG/compare/v0.6.2...v0.6.3) (2026-10-30)

### Performance Improvements

* release builds use one codegen unit and no symbols ([d9b2435](https://github.com/enke-dev/E-RNG/commit/d9b24353fa58e0efe6242b1e2f6fe7e6347cea90))
* the mustang jar leaves out what the app never runs ([e9f7606](https://github.com/enke-dev/E-RNG/commit/e9f7606042b3af8cda6561cff1c2e3fe064fa833))
* typst is compiled into the app, no sidecar ([739c0ab](https://github.com/enke-dev/E-RNG/commit/739c0abce74b34d44aa550be217e79d2dc472ce3))

## [0.6.2](https://github.com/enke-dev/E-RNG/compare/v0.6.1...v0.6.2) (2026-10-29)

### Bug Fixes

* an update installs when the user says so ([bcc5c4b](https://github.com/enke-dev/E-RNG/commit/bcc5c4b8e96bbfaf266fd7146ef3f8138714dc97))

## [0.6.1](https://github.com/enke-dev/E-RNG/compare/v0.6.0...v0.6.1) (2026-10-29)

### Bug Fixes

* no space between a number and its unit in the app ([1ed81f0](https://github.com/enke-dev/E-RNG/commit/1ed81f014a6935147cc5fd4e942893dd1fcc5ce5))
* vat rows list the highest rate first; an interrupted restore is shown ([f74b875](https://github.com/enke-dev/E-RNG/commit/f74b8753fa53a9d0ec57ed9fb95973cc2f828193))

# [0.6.0](https://github.com/enke-dev/E-RNG/compare/v0.5.2...v0.6.0) (2026-10-29)

### Bug Fixes

* a customer without an address leaves no empty elements in the XML ([36633c7](https://github.com/enke-dev/E-RNG/commit/36633c79e0edb03a7d9dc3b27c00a88f448214d4))
* fonts with Mac-only names and font collections import fully ([2c5b1ab](https://github.com/enke-dev/E-RNG/commit/2c5b1abc3d5b8dbcacc4c3262a6a257273393123))
* layouts are tagged integriert or individuell, their parts are Elemente ([ec48bad](https://github.com/enke-dev/E-RNG/commit/ec48badf3493e89f1061802f8deac44507a4b446))
* part boxes are read with typst eval, query is deprecated ([f849ecc](https://github.com/enke-dev/E-RNG/commit/f849ecc0cecaa1eb812bb7e31b1f7b8746014865))
* the layout editor keeps its place when saving and switching pages ([4ab4255](https://github.com/enke-dev/E-RNG/commit/4ab4255ff3f646f08b65399435a41025e22cd4f6))

### Features

* a customer address is required above 250 euros gross ([0a01e72](https://github.com/enke-dev/E-RNG/commit/0a01e72c6ec103a420c7cc77ccbbf4dad7eb515e))
* a debug build can start from another data folder ([83b195e](https://github.com/enke-dev/E-RNG/commit/83b195e7aa5c4d617402990c9a76a8fe9fee16cb))
* an invoice can name its offer ([ff73fae](https://github.com/enke-dev/E-RNG/commit/ff73faeb347da1ade4ce019e4c1560185c4f5045))
* back up and restore all data ([4845e24](https://github.com/enke-dev/E-RNG/commit/4845e24945ca89188f10eb475e15423a73935708))
* drafts read as Rechnungsentwurf and export as PDF ([243e3a0](https://github.com/enke-dev/E-RNG/commit/243e3a02dcd50877be85a8b5d38db5f1ba94e212))
* drafts save themselves ([8202d13](https://github.com/enke-dev/E-RNG/commit/8202d13d24d7200be7b67c92917c9a0ebed3ded3))
* invoice list filters by status and shows archived invoices ([b9f1247](https://github.com/enke-dev/E-RNG/commit/b9f1247d739e7da92f81be5fd8818c0000f32ea9))
* layout lines, texts and other parts take a color ([902df2f](https://github.com/enke-dev/E-RNG/commit/902df2f85af84683f44652e2d6e69e753dd05728)), closes [#rrggbb](https://github.com/enke-dev/E-RNG/issues/rrggbb)
* layout parts can be reordered ([b5e147f](https://github.com/enke-dev/E-RNG/commit/b5e147fcec4126bd5f959659354f1b3a4be1374d))
* layouts fill single seller fields; sellers get mobile, fax, website and bank name ([280e10f](https://github.com/enke-dev/E-RNG/commit/280e10fad2be44adcb7aa5991b4213a7868bd842))
* number sequences continue from a start number and by fiscal year ([87ed0f1](https://github.com/enke-dev/E-RNG/commit/87ed0f1c294c9aeeef05739d3f3a5b22a8262295))
* offers and invoices are linked, an offer becomes an invoice draft ([3bc16cf](https://github.com/enke-dev/E-RNG/commit/3bc16cfb03040384eededafef9e51547cececdc9))
* offers as a second kind of document ([8e895e0](https://github.com/enke-dev/E-RNG/commit/8e895e0ad1a67d48059cb30a10249bb10891091e))
* offers get their own list, editor and settings ([0912b99](https://github.com/enke-dev/E-RNG/commit/0912b991d05172862a77e71c5279ae46a1eaf9ad))
* service as a date or a period ([b75dc6d](https://github.com/enke-dev/E-RNG/commit/b75dc6d2c31b643f8c2367afeb8994a7b003863a))
* the layout preview shows invisible characters with its guides ([06cf9b6](https://github.com/enke-dev/E-RNG/commit/06cf9b670d57deb4ddaf8b8a5db4e897f6448a5b))
* the page number can stay away from single pages ([b0fa3fd](https://github.com/enke-dev/E-RNG/commit/b0fa3fd6a85d4db6b51c0eb7b6f8d747a65bad0d))
* the positions show the VAT rate in a column of its own ([bcf4a55](https://github.com/enke-dev/E-RNG/commit/bcf4a55a4d9bb0bac111a0d26a2ef0d656f8b7a9))
* units are configured per context ([41a9d7f](https://github.com/enke-dev/E-RNG/commit/41a9d7f940d62af5e0930d3675ca96cdd27ff996))
* **updater:** spin in a square only while nothing can be counted ([5a439c8](https://github.com/enke-dev/E-RNG/commit/5a439c8dd46303a3c656bd4eedac99fe01cbac77))
* vat rates per context and position ([e1f7877](https://github.com/enke-dev/E-RNG/commit/e1f7877e38ca1a8928e34a12d96522a7dd28f6fa))

## [0.5.2](https://github.com/enke-dev/E-RNG/compare/v0.5.1...v0.5.2) (2026-10-26)

### Bug Fixes

* search of customers and positions matches the context names ([5cf9a5d](https://github.com/enke-dev/E-RNG/commit/5cf9a5ddb2ee1c6d8701458acf6f572b018f5109))

## [0.5.1](https://github.com/enke-dev/E-RNG/compare/v0.5.0...v0.5.1) (2026-10-26)

### Bug Fixes

* add missing Einstellungen in the menu bar ([26fdeba](https://github.com/enke-dev/E-RNG/commit/26fdebab4fe616d7f4d445a65e8b0997059fc44e))
* fonts stored before their style was read get it from the file ([5025453](https://github.com/enke-dev/E-RNG/commit/5025453132186164d755ed090ee7691c5932f289))

# [0.5.0](https://github.com/enke-dev/E-RNG/compare/v0.4.0...v0.5.0) (2026-10-24)

### Features

* create invoices in one step and keep their originals ([ddd7f9f](https://github.com/enke-dev/E-RNG/commit/ddd7f9faeb9c24782727fc4b1fe869e6ce13289a))
* update progress in the button and a menu entry to look for updates ([771765e](https://github.com/enke-dev/E-RNG/commit/771765ecb08cfa0bce1b5eff51665db01f302979))

# [0.4.0](https://github.com/enke-dev/E-RNG/compare/v0.3.1...v0.4.0) (2026-10-20)

### Bug Fixes

* the divider of a split panel stays behind the page footer ([4d3977b](https://github.com/enke-dev/E-RNG/commit/4d3977b9b0d75aa7fea9fb78b8fac7c44d8182e8))

### Features

* a tax number alone identifies the seller ([d640f71](https://github.com/enke-dev/E-RNG/commit/d640f7156e3f4da07b26a03f677a67bb6c76a2bc))
* contexts for several issuers ([ebb4ba3](https://github.com/enke-dev/E-RNG/commit/ebb4ba39f31a81cac4edc69f053e2a6197b9ee44))
* guides show the printable area and the punch holes, and are remembered ([1362868](https://github.com/enke-dev/E-RNG/commit/136286828a195f4eeb79e31623d52a57c18c6ed4))
* invoice numbers follow a pattern, legal information is one field ([18fa96b](https://github.com/enke-dev/E-RNG/commit/18fa96bce183346ab70e53b5ab4e52464533583f))
* invoices without VAT and without payment term ([ed56c8a](https://github.com/enke-dev/E-RNG/commit/ed56c8a7acbcb2080d1e6e1dd1c857929a018ea9))
* layouts can be exported to and imported from a file ([c5bb1e8](https://github.com/enke-dev/E-RNG/commit/c5bb1e8634c453fa7f5152cde7e510ec76308fa9))
* let the user move the divider and restore the window at start ([b4d241f](https://github.com/enke-dev/E-RNG/commit/b4d241fb9ecc798a3a83aeba468bb54cbf0bb7d7))
* tell the hovered part from the selected one ([1df7f9c](https://github.com/enke-dev/E-RNG/commit/1df7f9cb940b58d4b88775313d3a73dd85c7df48))

## [0.3.1](https://github.com/enke-dev/E-RNG/compare/v0.3.0...v0.3.1) (2026-10-19)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

## [0.6.2](https://github.com/enke-dev/E-RNG/compare/v0.6.1...v0.6.2) (2026-10-29)

### Bug Fixes

* an update installs when the user says so ([bcc5c4b](https://github.com/enke-dev/E-RNG/commit/bcc5c4b8e96bbfaf266fd7146ef3f8138714dc97))

## [0.6.1](https://github.com/enke-dev/E-RNG/compare/v0.6.0...v0.6.1) (2026-10-29)

### Bug Fixes

* no space between a number and its unit in the app ([1ed81f0](https://github.com/enke-dev/E-RNG/commit/1ed81f014a6935147cc5fd4e942893dd1fcc5ce5))
* vat rows list the highest rate first; an interrupted restore is shown ([f74b875](https://github.com/enke-dev/E-RNG/commit/f74b8753fa53a9d0ec57ed9fb95973cc2f828193))

# [0.6.0](https://github.com/enke-dev/E-RNG/compare/v0.5.2...v0.6.0) (2026-10-29)

### Bug Fixes

* a customer without an address leaves no empty elements in the XML ([36633c7](https://github.com/enke-dev/E-RNG/commit/36633c79e0edb03a7d9dc3b27c00a88f448214d4))
* fonts with Mac-only names and font collections import fully ([2c5b1ab](https://github.com/enke-dev/E-RNG/commit/2c5b1abc3d5b8dbcacc4c3262a6a257273393123))
* layouts are tagged integriert or individuell, their parts are Elemente ([ec48bad](https://github.com/enke-dev/E-RNG/commit/ec48badf3493e89f1061802f8deac44507a4b446))
* part boxes are read with typst eval, query is deprecated ([f849ecc](https://github.com/enke-dev/E-RNG/commit/f849ecc0cecaa1eb812bb7e31b1f7b8746014865))
* the layout editor keeps its place when saving and switching pages ([4ab4255](https://github.com/enke-dev/E-RNG/commit/4ab4255ff3f646f08b65399435a41025e22cd4f6))

### Features

* a customer address is required above 250 euros gross ([0a01e72](https://github.com/enke-dev/E-RNG/commit/0a01e72c6ec103a420c7cc77ccbbf4dad7eb515e))
* a debug build can start from another data folder ([83b195e](https://github.com/enke-dev/E-RNG/commit/83b195e7aa5c4d617402990c9a76a8fe9fee16cb))
* an invoice can name its offer ([ff73fae](https://github.com/enke-dev/E-RNG/commit/ff73faeb347da1ade4ce019e4c1560185c4f5045))
* back up and restore all data ([4845e24](https://github.com/enke-dev/E-RNG/commit/4845e24945ca89188f10eb475e15423a73935708))
* drafts read as Rechnungsentwurf and export as PDF ([243e3a0](https://github.com/enke-dev/E-RNG/commit/243e3a02dcd50877be85a8b5d38db5f1ba94e212))
* drafts save themselves ([8202d13](https://github.com/enke-dev/E-RNG/commit/8202d13d24d7200be7b67c92917c9a0ebed3ded3))
* invoice list filters by status and shows archived invoices ([b9f1247](https://github.com/enke-dev/E-RNG/commit/b9f1247d739e7da92f81be5fd8818c0000f32ea9))
* layout lines, texts and other parts take a color ([902df2f](https://github.com/enke-dev/E-RNG/commit/902df2f85af84683f44652e2d6e69e753dd05728)), closes [#rrggbb](https://github.com/enke-dev/E-RNG/issues/rrggbb)
* layout parts can be reordered ([b5e147f](https://github.com/enke-dev/E-RNG/commit/b5e147fcec4126bd5f959659354f1b3a4be1374d))
* layouts fill single seller fields; sellers get mobile, fax, website and bank name ([280e10f](https://github.com/enke-dev/E-RNG/commit/280e10fad2be44adcb7aa5991b4213a7868bd842))
* number sequences continue from a start number and by fiscal year ([87ed0f1](https://github.com/enke-dev/E-RNG/commit/87ed0f1c294c9aeeef05739d3f3a5b22a8262295))
* offers and invoices are linked, an offer becomes an invoice draft ([3bc16cf](https://github.com/enke-dev/E-RNG/commit/3bc16cfb03040384eededafef9e51547cececdc9))
* offers as a second kind of document ([8e895e0](https://github.com/enke-dev/E-RNG/commit/8e895e0ad1a67d48059cb30a10249bb10891091e))
* offers get their own list, editor and settings ([0912b99](https://github.com/enke-dev/E-RNG/commit/0912b991d05172862a77e71c5279ae46a1eaf9ad))
* service as a date or a period ([b75dc6d](https://github.com/enke-dev/E-RNG/commit/b75dc6d2c31b643f8c2367afeb8994a7b003863a))
* the layout preview shows invisible characters with its guides ([06cf9b6](https://github.com/enke-dev/E-RNG/commit/06cf9b670d57deb4ddaf8b8a5db4e897f6448a5b))
* the page number can stay away from single pages ([b0fa3fd](https://github.com/enke-dev/E-RNG/commit/b0fa3fd6a85d4db6b51c0eb7b6f8d747a65bad0d))
* the positions show the VAT rate in a column of its own ([bcf4a55](https://github.com/enke-dev/E-RNG/commit/bcf4a55a4d9bb0bac111a0d26a2ef0d656f8b7a9))
* units are configured per context ([41a9d7f](https://github.com/enke-dev/E-RNG/commit/41a9d7f940d62af5e0930d3675ca96cdd27ff996))
* **updater:** spin in a square only while nothing can be counted ([5a439c8](https://github.com/enke-dev/E-RNG/commit/5a439c8dd46303a3c656bd4eedac99fe01cbac77))
* vat rates per context and position ([e1f7877](https://github.com/enke-dev/E-RNG/commit/e1f7877e38ca1a8928e34a12d96522a7dd28f6fa))

## [0.5.2](https://github.com/enke-dev/E-RNG/compare/v0.5.1...v0.5.2) (2026-10-26)

### Bug Fixes

* search of customers and positions matches the context names ([5cf9a5d](https://github.com/enke-dev/E-RNG/commit/5cf9a5ddb2ee1c6d8701458acf6f572b018f5109))

## [0.5.1](https://github.com/enke-dev/E-RNG/compare/v0.5.0...v0.5.1) (2026-10-26)

### Bug Fixes

* add missing Einstellungen in the menu bar ([26fdeba](https://github.com/enke-dev/E-RNG/commit/26fdebab4fe616d7f4d445a65e8b0997059fc44e))
* fonts stored before their style was read get it from the file ([5025453](https://github.com/enke-dev/E-RNG/commit/5025453132186164d755ed090ee7691c5932f289))

# [0.5.0](https://github.com/enke-dev/E-RNG/compare/v0.4.0...v0.5.0) (2026-10-24)

### Features

* create invoices in one step and keep their originals ([ddd7f9f](https://github.com/enke-dev/E-RNG/commit/ddd7f9faeb9c24782727fc4b1fe869e6ce13289a))
* update progress in the button and a menu entry to look for updates ([771765e](https://github.com/enke-dev/E-RNG/commit/771765ecb08cfa0bce1b5eff51665db01f302979))

# [0.4.0](https://github.com/enke-dev/E-RNG/compare/v0.3.1...v0.4.0) (2026-10-20)

### Bug Fixes

* the divider of a split panel stays behind the page footer ([4d3977b](https://github.com/enke-dev/E-RNG/commit/4d3977b9b0d75aa7fea9fb78b8fac7c44d8182e8))

### Features

* a tax number alone identifies the seller ([d640f71](https://github.com/enke-dev/E-RNG/commit/d640f7156e3f4da07b26a03f677a67bb6c76a2bc))
* contexts for several issuers ([ebb4ba3](https://github.com/enke-dev/E-RNG/commit/ebb4ba39f31a81cac4edc69f053e2a6197b9ee44))
* guides show the printable area and the punch holes, and are remembered ([1362868](https://github.com/enke-dev/E-RNG/commit/136286828a195f4eeb79e31623d52a57c18c6ed4))
* invoice numbers follow a pattern, legal information is one field ([18fa96b](https://github.com/enke-dev/E-RNG/commit/18fa96bce183346ab70e53b5ab4e52464533583f))
* invoices without VAT and without payment term ([ed56c8a](https://github.com/enke-dev/E-RNG/commit/ed56c8a7acbcb2080d1e6e1dd1c857929a018ea9))
* layouts can be exported to and imported from a file ([c5bb1e8](https://github.com/enke-dev/E-RNG/commit/c5bb1e8634c453fa7f5152cde7e510ec76308fa9))
* let the user move the divider and restore the window at start ([b4d241f](https://github.com/enke-dev/E-RNG/commit/b4d241fb9ecc798a3a83aeba468bb54cbf0bb7d7))
* tell the hovered part from the selected one ([1df7f9c](https://github.com/enke-dev/E-RNG/commit/1df7f9cb940b58d4b88775313d3a73dd85c7df48))

## [0.3.1](https://github.com/enke-dev/E-RNG/compare/v0.3.0...v0.3.1) (2026-10-19)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

## [0.6.1](https://github.com/enke-dev/E-RNG/compare/v0.6.0...v0.6.1) (2026-10-29)

### Bug Fixes

* no space between a number and its unit in the app ([1ed81f0](https://github.com/enke-dev/E-RNG/commit/1ed81f014a6935147cc5fd4e942893dd1fcc5ce5))
* vat rows list the highest rate first; an interrupted restore is shown ([f74b875](https://github.com/enke-dev/E-RNG/commit/f74b8753fa53a9d0ec57ed9fb95973cc2f828193))

# [0.6.0](https://github.com/enke-dev/E-RNG/compare/v0.5.2...v0.6.0) (2026-10-29)

### Bug Fixes

* a customer without an address leaves no empty elements in the XML ([36633c7](https://github.com/enke-dev/E-RNG/commit/36633c79e0edb03a7d9dc3b27c00a88f448214d4))
* fonts with Mac-only names and font collections import fully ([2c5b1ab](https://github.com/enke-dev/E-RNG/commit/2c5b1abc3d5b8dbcacc4c3262a6a257273393123))
* layouts are tagged integriert or individuell, their parts are Elemente ([ec48bad](https://github.com/enke-dev/E-RNG/commit/ec48badf3493e89f1061802f8deac44507a4b446))
* part boxes are read with typst eval, query is deprecated ([f849ecc](https://github.com/enke-dev/E-RNG/commit/f849ecc0cecaa1eb812bb7e31b1f7b8746014865))
* the layout editor keeps its place when saving and switching pages ([4ab4255](https://github.com/enke-dev/E-RNG/commit/4ab4255ff3f646f08b65399435a41025e22cd4f6))

### Features

* a customer address is required above 250 euros gross ([0a01e72](https://github.com/enke-dev/E-RNG/commit/0a01e72c6ec103a420c7cc77ccbbf4dad7eb515e))
* a debug build can start from another data folder ([83b195e](https://github.com/enke-dev/E-RNG/commit/83b195e7aa5c4d617402990c9a76a8fe9fee16cb))
* an invoice can name its offer ([ff73fae](https://github.com/enke-dev/E-RNG/commit/ff73faeb347da1ade4ce019e4c1560185c4f5045))
* back up and restore all data ([4845e24](https://github.com/enke-dev/E-RNG/commit/4845e24945ca89188f10eb475e15423a73935708))
* drafts read as Rechnungsentwurf and export as PDF ([243e3a0](https://github.com/enke-dev/E-RNG/commit/243e3a02dcd50877be85a8b5d38db5f1ba94e212))
* drafts save themselves ([8202d13](https://github.com/enke-dev/E-RNG/commit/8202d13d24d7200be7b67c92917c9a0ebed3ded3))
* invoice list filters by status and shows archived invoices ([b9f1247](https://github.com/enke-dev/E-RNG/commit/b9f1247d739e7da92f81be5fd8818c0000f32ea9))
* layout lines, texts and other parts take a color ([902df2f](https://github.com/enke-dev/E-RNG/commit/902df2f85af84683f44652e2d6e69e753dd05728)), closes [#rrggbb](https://github.com/enke-dev/E-RNG/issues/rrggbb)
* layout parts can be reordered ([b5e147f](https://github.com/enke-dev/E-RNG/commit/b5e147fcec4126bd5f959659354f1b3a4be1374d))
* layouts fill single seller fields; sellers get mobile, fax, website and bank name ([280e10f](https://github.com/enke-dev/E-RNG/commit/280e10fad2be44adcb7aa5991b4213a7868bd842))
* number sequences continue from a start number and by fiscal year ([87ed0f1](https://github.com/enke-dev/E-RNG/commit/87ed0f1c294c9aeeef05739d3f3a5b22a8262295))
* offers and invoices are linked, an offer becomes an invoice draft ([3bc16cf](https://github.com/enke-dev/E-RNG/commit/3bc16cfb03040384eededafef9e51547cececdc9))
* offers as a second kind of document ([8e895e0](https://github.com/enke-dev/E-RNG/commit/8e895e0ad1a67d48059cb30a10249bb10891091e))
* offers get their own list, editor and settings ([0912b99](https://github.com/enke-dev/E-RNG/commit/0912b991d05172862a77e71c5279ae46a1eaf9ad))
* service as a date or a period ([b75dc6d](https://github.com/enke-dev/E-RNG/commit/b75dc6d2c31b643f8c2367afeb8994a7b003863a))
* the layout preview shows invisible characters with its guides ([06cf9b6](https://github.com/enke-dev/E-RNG/commit/06cf9b670d57deb4ddaf8b8a5db4e897f6448a5b))
* the page number can stay away from single pages ([b0fa3fd](https://github.com/enke-dev/E-RNG/commit/b0fa3fd6a85d4db6b51c0eb7b6f8d747a65bad0d))
* the positions show the VAT rate in a column of its own ([bcf4a55](https://github.com/enke-dev/E-RNG/commit/bcf4a55a4d9bb0bac111a0d26a2ef0d656f8b7a9))
* units are configured per context ([41a9d7f](https://github.com/enke-dev/E-RNG/commit/41a9d7f940d62af5e0930d3675ca96cdd27ff996))
* **updater:** spin in a square only while nothing can be counted ([5a439c8](https://github.com/enke-dev/E-RNG/commit/5a439c8dd46303a3c656bd4eedac99fe01cbac77))
* vat rates per context and position ([e1f7877](https://github.com/enke-dev/E-RNG/commit/e1f7877e38ca1a8928e34a12d96522a7dd28f6fa))

## [0.5.2](https://github.com/enke-dev/E-RNG/compare/v0.5.1...v0.5.2) (2026-10-26)

### Bug Fixes

* search of customers and positions matches the context names ([5cf9a5d](https://github.com/enke-dev/E-RNG/commit/5cf9a5ddb2ee1c6d8701458acf6f572b018f5109))

## [0.5.1](https://github.com/enke-dev/E-RNG/compare/v0.5.0...v0.5.1) (2026-10-26)

### Bug Fixes

* add missing Einstellungen in the menu bar ([26fdeba](https://github.com/enke-dev/E-RNG/commit/26fdebab4fe616d7f4d445a65e8b0997059fc44e))
* fonts stored before their style was read get it from the file ([5025453](https://github.com/enke-dev/E-RNG/commit/5025453132186164d755ed090ee7691c5932f289))

# [0.5.0](https://github.com/enke-dev/E-RNG/compare/v0.4.0...v0.5.0) (2026-10-24)

### Features

* create invoices in one step and keep their originals ([ddd7f9f](https://github.com/enke-dev/E-RNG/commit/ddd7f9faeb9c24782727fc4b1fe869e6ce13289a))
* update progress in the button and a menu entry to look for updates ([771765e](https://github.com/enke-dev/E-RNG/commit/771765ecb08cfa0bce1b5eff51665db01f302979))

# [0.4.0](https://github.com/enke-dev/E-RNG/compare/v0.3.1...v0.4.0) (2026-10-20)

### Bug Fixes

* the divider of a split panel stays behind the page footer ([4d3977b](https://github.com/enke-dev/E-RNG/commit/4d3977b9b0d75aa7fea9fb78b8fac7c44d8182e8))

### Features

* a tax number alone identifies the seller ([d640f71](https://github.com/enke-dev/E-RNG/commit/d640f7156e3f4da07b26a03f677a67bb6c76a2bc))
* contexts for several issuers ([ebb4ba3](https://github.com/enke-dev/E-RNG/commit/ebb4ba39f31a81cac4edc69f053e2a6197b9ee44))
* guides show the printable area and the punch holes, and are remembered ([1362868](https://github.com/enke-dev/E-RNG/commit/136286828a195f4eeb79e31623d52a57c18c6ed4))
* invoice numbers follow a pattern, legal information is one field ([18fa96b](https://github.com/enke-dev/E-RNG/commit/18fa96bce183346ab70e53b5ab4e52464533583f))
* invoices without VAT and without payment term ([ed56c8a](https://github.com/enke-dev/E-RNG/commit/ed56c8a7acbcb2080d1e6e1dd1c857929a018ea9))
* layouts can be exported to and imported from a file ([c5bb1e8](https://github.com/enke-dev/E-RNG/commit/c5bb1e8634c453fa7f5152cde7e510ec76308fa9))
* let the user move the divider and restore the window at start ([b4d241f](https://github.com/enke-dev/E-RNG/commit/b4d241fb9ecc798a3a83aeba468bb54cbf0bb7d7))
* tell the hovered part from the selected one ([1df7f9c](https://github.com/enke-dev/E-RNG/commit/1df7f9cb940b58d4b88775313d3a73dd85c7df48))

## [0.3.1](https://github.com/enke-dev/E-RNG/compare/v0.3.0...v0.3.1) (2026-10-19)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

# [0.6.0](https://github.com/enke-dev/E-RNG/compare/v0.5.2...v0.6.0) (2026-10-29)

### Bug Fixes

* a customer without an address leaves no empty elements in the XML ([36633c7](https://github.com/enke-dev/E-RNG/commit/36633c79e0edb03a7d9dc3b27c00a88f448214d4))
* fonts with Mac-only names and font collections import fully ([2c5b1ab](https://github.com/enke-dev/E-RNG/commit/2c5b1abc3d5b8dbcacc4c3262a6a257273393123))
* layouts are tagged integriert or individuell, their parts are Elemente ([ec48bad](https://github.com/enke-dev/E-RNG/commit/ec48badf3493e89f1061802f8deac44507a4b446))
* part boxes are read with typst eval, query is deprecated ([f849ecc](https://github.com/enke-dev/E-RNG/commit/f849ecc0cecaa1eb812bb7e31b1f7b8746014865))
* the layout editor keeps its place when saving and switching pages ([4ab4255](https://github.com/enke-dev/E-RNG/commit/4ab4255ff3f646f08b65399435a41025e22cd4f6))

### Features

* a customer address is required above 250 euros gross ([0a01e72](https://github.com/enke-dev/E-RNG/commit/0a01e72c6ec103a420c7cc77ccbbf4dad7eb515e))
* a debug build can start from another data folder ([83b195e](https://github.com/enke-dev/E-RNG/commit/83b195e7aa5c4d617402990c9a76a8fe9fee16cb))
* an invoice can name its offer ([ff73fae](https://github.com/enke-dev/E-RNG/commit/ff73faeb347da1ade4ce019e4c1560185c4f5045))
* back up and restore all data ([4845e24](https://github.com/enke-dev/E-RNG/commit/4845e24945ca89188f10eb475e15423a73935708))
* drafts read as Rechnungsentwurf and export as PDF ([243e3a0](https://github.com/enke-dev/E-RNG/commit/243e3a02dcd50877be85a8b5d38db5f1ba94e212))
* drafts save themselves ([8202d13](https://github.com/enke-dev/E-RNG/commit/8202d13d24d7200be7b67c92917c9a0ebed3ded3))
* invoice list filters by status and shows archived invoices ([b9f1247](https://github.com/enke-dev/E-RNG/commit/b9f1247d739e7da92f81be5fd8818c0000f32ea9))
* layout lines, texts and other parts take a color ([902df2f](https://github.com/enke-dev/E-RNG/commit/902df2f85af84683f44652e2d6e69e753dd05728)), closes [#rrggbb](https://github.com/enke-dev/E-RNG/issues/rrggbb)
* layout parts can be reordered ([b5e147f](https://github.com/enke-dev/E-RNG/commit/b5e147fcec4126bd5f959659354f1b3a4be1374d))
* layouts fill single seller fields; sellers get mobile, fax, website and bank name ([280e10f](https://github.com/enke-dev/E-RNG/commit/280e10fad2be44adcb7aa5991b4213a7868bd842))
* number sequences continue from a start number and by fiscal year ([87ed0f1](https://github.com/enke-dev/E-RNG/commit/87ed0f1c294c9aeeef05739d3f3a5b22a8262295))
* offers and invoices are linked, an offer becomes an invoice draft ([3bc16cf](https://github.com/enke-dev/E-RNG/commit/3bc16cfb03040384eededafef9e51547cececdc9))
* offers as a second kind of document ([8e895e0](https://github.com/enke-dev/E-RNG/commit/8e895e0ad1a67d48059cb30a10249bb10891091e))
* offers get their own list, editor and settings ([0912b99](https://github.com/enke-dev/E-RNG/commit/0912b991d05172862a77e71c5279ae46a1eaf9ad))
* service as a date or a period ([b75dc6d](https://github.com/enke-dev/E-RNG/commit/b75dc6d2c31b643f8c2367afeb8994a7b003863a))
* the layout preview shows invisible characters with its guides ([06cf9b6](https://github.com/enke-dev/E-RNG/commit/06cf9b670d57deb4ddaf8b8a5db4e897f6448a5b))
* the page number can stay away from single pages ([b0fa3fd](https://github.com/enke-dev/E-RNG/commit/b0fa3fd6a85d4db6b51c0eb7b6f8d747a65bad0d))
* the positions show the VAT rate in a column of its own ([bcf4a55](https://github.com/enke-dev/E-RNG/commit/bcf4a55a4d9bb0bac111a0d26a2ef0d656f8b7a9))
* units are configured per context ([41a9d7f](https://github.com/enke-dev/E-RNG/commit/41a9d7f940d62af5e0930d3675ca96cdd27ff996))
* **updater:** spin in a square only while nothing can be counted ([5a439c8](https://github.com/enke-dev/E-RNG/commit/5a439c8dd46303a3c656bd4eedac99fe01cbac77))
* vat rates per context and position ([e1f7877](https://github.com/enke-dev/E-RNG/commit/e1f7877e38ca1a8928e34a12d96522a7dd28f6fa))

## [0.5.2](https://github.com/enke-dev/E-RNG/compare/v0.5.1...v0.5.2) (2026-10-26)

### Bug Fixes

* search of customers and positions matches the context names ([5cf9a5d](https://github.com/enke-dev/E-RNG/commit/5cf9a5ddb2ee1c6d8701458acf6f572b018f5109))

## [0.5.1](https://github.com/enke-dev/E-RNG/compare/v0.5.0...v0.5.1) (2026-10-26)

### Bug Fixes

* add missing Einstellungen in the menu bar ([26fdeba](https://github.com/enke-dev/E-RNG/commit/26fdebab4fe616d7f4d445a65e8b0997059fc44e))
* fonts stored before their style was read get it from the file ([5025453](https://github.com/enke-dev/E-RNG/commit/5025453132186164d755ed090ee7691c5932f289))

# [0.5.0](https://github.com/enke-dev/E-RNG/compare/v0.4.0...v0.5.0) (2026-10-24)

### Features

* create invoices in one step and keep their originals ([ddd7f9f](https://github.com/enke-dev/E-RNG/commit/ddd7f9faeb9c24782727fc4b1fe869e6ce13289a))
* update progress in the button and a menu entry to look for updates ([771765e](https://github.com/enke-dev/E-RNG/commit/771765ecb08cfa0bce1b5eff51665db01f302979))

# [0.4.0](https://github.com/enke-dev/E-RNG/compare/v0.3.1...v0.4.0) (2026-10-20)

### Bug Fixes

* the divider of a split panel stays behind the page footer ([4d3977b](https://github.com/enke-dev/E-RNG/commit/4d3977b9b0d75aa7fea9fb78b8fac7c44d8182e8))

### Features

* a tax number alone identifies the seller ([d640f71](https://github.com/enke-dev/E-RNG/commit/d640f7156e3f4da07b26a03f677a67bb6c76a2bc))
* contexts for several issuers ([ebb4ba3](https://github.com/enke-dev/E-RNG/commit/ebb4ba39f31a81cac4edc69f053e2a6197b9ee44))
* guides show the printable area and the punch holes, and are remembered ([1362868](https://github.com/enke-dev/E-RNG/commit/136286828a195f4eeb79e31623d52a57c18c6ed4))
* invoice numbers follow a pattern, legal information is one field ([18fa96b](https://github.com/enke-dev/E-RNG/commit/18fa96bce183346ab70e53b5ab4e52464533583f))
* invoices without VAT and without payment term ([ed56c8a](https://github.com/enke-dev/E-RNG/commit/ed56c8a7acbcb2080d1e6e1dd1c857929a018ea9))
* layouts can be exported to and imported from a file ([c5bb1e8](https://github.com/enke-dev/E-RNG/commit/c5bb1e8634c453fa7f5152cde7e510ec76308fa9))
* let the user move the divider and restore the window at start ([b4d241f](https://github.com/enke-dev/E-RNG/commit/b4d241fb9ecc798a3a83aeba468bb54cbf0bb7d7))
* tell the hovered part from the selected one ([1df7f9c](https://github.com/enke-dev/E-RNG/commit/1df7f9cb940b58d4b88775313d3a73dd85c7df48))

## [0.3.1](https://github.com/enke-dev/E-RNG/compare/v0.3.0...v0.3.1) (2026-10-19)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

## [0.5.2](https://github.com/enke-dev/E-RNG/compare/v0.5.1...v0.5.2) (2026-10-26)

### Bug Fixes

* search of customers and positions matches the context names ([5cf9a5d](https://github.com/enke-dev/E-RNG/commit/5cf9a5ddb2ee1c6d8701458acf6f572b018f5109))

## [0.5.1](https://github.com/enke-dev/E-RNG/compare/v0.5.0...v0.5.1) (2026-10-26)

### Bug Fixes

* add missing Einstellungen in the menu bar ([26fdeba](https://github.com/enke-dev/E-RNG/commit/26fdebab4fe616d7f4d445a65e8b0997059fc44e))
* fonts stored before their style was read get it from the file ([5025453](https://github.com/enke-dev/E-RNG/commit/5025453132186164d755ed090ee7691c5932f289))

# [0.5.0](https://github.com/enke-dev/E-RNG/compare/v0.4.0...v0.5.0) (2026-10-24)

### Features

* create invoices in one step and keep their originals ([ddd7f9f](https://github.com/enke-dev/E-RNG/commit/ddd7f9faeb9c24782727fc4b1fe869e6ce13289a))
* update progress in the button and a menu entry to look for updates ([771765e](https://github.com/enke-dev/E-RNG/commit/771765ecb08cfa0bce1b5eff51665db01f302979))

# [0.4.0](https://github.com/enke-dev/E-RNG/compare/v0.3.1...v0.4.0) (2026-10-20)

### Bug Fixes

* the divider of a split panel stays behind the page footer ([4d3977b](https://github.com/enke-dev/E-RNG/commit/4d3977b9b0d75aa7fea9fb78b8fac7c44d8182e8))

### Features

* a tax number alone identifies the seller ([d640f71](https://github.com/enke-dev/E-RNG/commit/d640f7156e3f4da07b26a03f677a67bb6c76a2bc))
* contexts for several issuers ([ebb4ba3](https://github.com/enke-dev/E-RNG/commit/ebb4ba39f31a81cac4edc69f053e2a6197b9ee44))
* guides show the printable area and the punch holes, and are remembered ([1362868](https://github.com/enke-dev/E-RNG/commit/136286828a195f4eeb79e31623d52a57c18c6ed4))
* invoice numbers follow a pattern, legal information is one field ([18fa96b](https://github.com/enke-dev/E-RNG/commit/18fa96bce183346ab70e53b5ab4e52464533583f))
* invoices without VAT and without payment term ([ed56c8a](https://github.com/enke-dev/E-RNG/commit/ed56c8a7acbcb2080d1e6e1dd1c857929a018ea9))
* layouts can be exported to and imported from a file ([c5bb1e8](https://github.com/enke-dev/E-RNG/commit/c5bb1e8634c453fa7f5152cde7e510ec76308fa9))
* let the user move the divider and restore the window at start ([b4d241f](https://github.com/enke-dev/E-RNG/commit/b4d241fb9ecc798a3a83aeba468bb54cbf0bb7d7))
* tell the hovered part from the selected one ([1df7f9c](https://github.com/enke-dev/E-RNG/commit/1df7f9cb940b58d4b88775313d3a73dd85c7df48))

## [0.3.1](https://github.com/enke-dev/E-RNG/compare/v0.3.0...v0.3.1) (2026-10-19)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

## [0.5.1](https://github.com/enke-dev/E-RNG/compare/v0.5.0...v0.5.1) (2026-10-26)

### Bug Fixes

* add missing Einstellungen in the menu bar ([26fdeba](https://github.com/enke-dev/E-RNG/commit/26fdebab4fe616d7f4d445a65e8b0997059fc44e))
* fonts stored before their style was read get it from the file ([5025453](https://github.com/enke-dev/E-RNG/commit/5025453132186164d755ed090ee7691c5932f289))

# [0.5.0](https://github.com/enke-dev/E-RNG/compare/v0.4.0...v0.5.0) (2026-10-24)

### Features

* create invoices in one step and keep their originals ([ddd7f9f](https://github.com/enke-dev/E-RNG/commit/ddd7f9faeb9c24782727fc4b1fe869e6ce13289a))
* update progress in the button and a menu entry to look for updates ([771765e](https://github.com/enke-dev/E-RNG/commit/771765ecb08cfa0bce1b5eff51665db01f302979))

# [0.4.0](https://github.com/enke-dev/E-RNG/compare/v0.3.1...v0.4.0) (2026-10-20)

### Bug Fixes

* the divider of a split panel stays behind the page footer ([4d3977b](https://github.com/enke-dev/E-RNG/commit/4d3977b9b0d75aa7fea9fb78b8fac7c44d8182e8))

### Features

* a tax number alone identifies the seller ([d640f71](https://github.com/enke-dev/E-RNG/commit/d640f7156e3f4da07b26a03f677a67bb6c76a2bc))
* contexts for several issuers ([ebb4ba3](https://github.com/enke-dev/E-RNG/commit/ebb4ba39f31a81cac4edc69f053e2a6197b9ee44))
* guides show the printable area and the punch holes, and are remembered ([1362868](https://github.com/enke-dev/E-RNG/commit/136286828a195f4eeb79e31623d52a57c18c6ed4))
* invoice numbers follow a pattern, legal information is one field ([18fa96b](https://github.com/enke-dev/E-RNG/commit/18fa96bce183346ab70e53b5ab4e52464533583f))
* invoices without VAT and without payment term ([ed56c8a](https://github.com/enke-dev/E-RNG/commit/ed56c8a7acbcb2080d1e6e1dd1c857929a018ea9))
* layouts can be exported to and imported from a file ([c5bb1e8](https://github.com/enke-dev/E-RNG/commit/c5bb1e8634c453fa7f5152cde7e510ec76308fa9))
* let the user move the divider and restore the window at start ([b4d241f](https://github.com/enke-dev/E-RNG/commit/b4d241fb9ecc798a3a83aeba468bb54cbf0bb7d7))
* tell the hovered part from the selected one ([1df7f9c](https://github.com/enke-dev/E-RNG/commit/1df7f9cb940b58d4b88775313d3a73dd85c7df48))

## [0.3.1](https://github.com/enke-dev/E-RNG/compare/v0.3.0...v0.3.1) (2026-10-19)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

# [0.5.0](https://github.com/enke-dev/E-RNG/compare/v0.4.0...v0.5.0) (2026-10-24)

### Features

* create invoices in one step and keep their originals ([ddd7f9f](https://github.com/enke-dev/E-RNG/commit/ddd7f9faeb9c24782727fc4b1fe869e6ce13289a))
* update progress in the button and a menu entry to look for updates ([771765e](https://github.com/enke-dev/E-RNG/commit/771765ecb08cfa0bce1b5eff51665db01f302979))

# [0.4.0](https://github.com/enke-dev/E-RNG/compare/v0.3.1...v0.4.0) (2026-10-20)

### Bug Fixes

* the divider of a split panel stays behind the page footer ([4d3977b](https://github.com/enke-dev/E-RNG/commit/4d3977b9b0d75aa7fea9fb78b8fac7c44d8182e8))

### Features

* a tax number alone identifies the seller ([d640f71](https://github.com/enke-dev/E-RNG/commit/d640f7156e3f4da07b26a03f677a67bb6c76a2bc))
* contexts for several issuers ([ebb4ba3](https://github.com/enke-dev/E-RNG/commit/ebb4ba39f31a81cac4edc69f053e2a6197b9ee44))
* guides show the printable area and the punch holes, and are remembered ([1362868](https://github.com/enke-dev/E-RNG/commit/136286828a195f4eeb79e31623d52a57c18c6ed4))
* invoice numbers follow a pattern, legal information is one field ([18fa96b](https://github.com/enke-dev/E-RNG/commit/18fa96bce183346ab70e53b5ab4e52464533583f))
* invoices without VAT and without payment term ([ed56c8a](https://github.com/enke-dev/E-RNG/commit/ed56c8a7acbcb2080d1e6e1dd1c857929a018ea9))
* layouts can be exported to and imported from a file ([c5bb1e8](https://github.com/enke-dev/E-RNG/commit/c5bb1e8634c453fa7f5152cde7e510ec76308fa9))
* let the user move the divider and restore the window at start ([b4d241f](https://github.com/enke-dev/E-RNG/commit/b4d241fb9ecc798a3a83aeba468bb54cbf0bb7d7))
* tell the hovered part from the selected one ([1df7f9c](https://github.com/enke-dev/E-RNG/commit/1df7f9cb940b58d4b88775313d3a73dd85c7df48))

## [0.3.1](https://github.com/enke-dev/E-RNG/compare/v0.3.0...v0.3.1) (2026-10-19)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

# [0.4.0](https://github.com/enke-dev/E-RNG/compare/v0.3.1...v0.4.0) (2026-10-20)

### Bug Fixes

* the divider of a split panel stays behind the page footer ([4d3977b](https://github.com/enke-dev/E-RNG/commit/4d3977b9b0d75aa7fea9fb78b8fac7c44d8182e8))

### Features

* a tax number alone identifies the seller ([d640f71](https://github.com/enke-dev/E-RNG/commit/d640f7156e3f4da07b26a03f677a67bb6c76a2bc))
* contexts for several issuers ([ebb4ba3](https://github.com/enke-dev/E-RNG/commit/ebb4ba39f31a81cac4edc69f053e2a6197b9ee44))
* guides show the printable area and the punch holes, and are remembered ([1362868](https://github.com/enke-dev/E-RNG/commit/136286828a195f4eeb79e31623d52a57c18c6ed4))
* invoice numbers follow a pattern, legal information is one field ([18fa96b](https://github.com/enke-dev/E-RNG/commit/18fa96bce183346ab70e53b5ab4e52464533583f))
* invoices without VAT and without payment term ([ed56c8a](https://github.com/enke-dev/E-RNG/commit/ed56c8a7acbcb2080d1e6e1dd1c857929a018ea9))
* layouts can be exported to and imported from a file ([c5bb1e8](https://github.com/enke-dev/E-RNG/commit/c5bb1e8634c453fa7f5152cde7e510ec76308fa9))
* let the user move the divider and restore the window at start ([b4d241f](https://github.com/enke-dev/E-RNG/commit/b4d241fb9ecc798a3a83aeba468bb54cbf0bb7d7))
* tell the hovered part from the selected one ([1df7f9c](https://github.com/enke-dev/E-RNG/commit/1df7f9cb940b58d4b88775313d3a73dd85c7df48))

## [0.3.1](https://github.com/enke-dev/E-RNG/compare/v0.3.0...v0.3.1) (2026-10-19)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

## [0.3.1](https://github.com/enke-dev/E-RNG/compare/v0.3.0...v0.3.1) (2026-10-19)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

# [0.3.0](https://github.com/enke-dev/E-RNG/compare/v0.2.1...v0.3.0) (2026-10-19)

### Features

* layouts are a frame and parts placed around it ([7a68ded](https://github.com/enke-dev/E-RNG/commit/7a68dedeeebb2f9bafc87521edc987b18e64b822))
* layouts decide how numbers, money and dates are written ([37a3029](https://github.com/enke-dev/E-RNG/commit/37a302975306f4f239e62fd2a578297f495d52ea))

## [0.2.1](https://github.com/enke-dev/E-RNG/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/E-RNG/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/E-RNG/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/E-RNG/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/E-RNG/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/E-RNG/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/E-RNG/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/E-RNG/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/E-RNG/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/E-RNG/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/E-RNG/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/E-RNG/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/E-RNG/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/E-RNG/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/E-RNG/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

## [0.2.1](https://github.com/enke-dev/e-rng/compare/v0.2.0...v0.2.1) (2026-10-18)

### Bug Fixes

* toasts for what just happened, a check badge for paid invoices, no duplicate templates ([bbba919](https://github.com/enke-dev/e-rng/commit/bbba91946c341fd54a27ec2a90bcab671d97382d))

# [0.2.0](https://github.com/enke-dev/e-rng/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/e-rng/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/e-rng/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/e-rng/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/e-rng/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/e-rng/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/e-rng/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/e-rng/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/e-rng/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/e-rng/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/e-rng/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/e-rng/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/e-rng/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

# [0.2.0](https://github.com/enke-dev/e-rng/compare/v0.1.0...v0.2.0) (2026-10-18)

### Bug Fixes

* debug builds keep their data apart from the installed app ([883c3cf](https://github.com/enke-dev/e-rng/commit/883c3cfae62053b7fa590d4a578351cb9e57a0ea))
* drag sorting swaps symmetrically with a dead band ([782d8bf](https://github.com/enke-dev/e-rng/commit/782d8bf9c97b64d7722e7ad4a721c35b0b8ee4b0))
* positions stack by measured need instead of a breakpoint ([dc7e8a9](https://github.com/enke-dev/e-rng/commit/dc7e8a96951f06e38641bc0dae1f04dd29e61919))
* the preview page fits the pane and its pager stays above the footer ([9385750](https://github.com/enke-dev/e-rng/commit/93857505b92d2f9138ea28bf89fcd04fbd487dc4))
* the settings can only be saved when something changed ([fc661f7](https://github.com/enke-dev/e-rng/commit/fc661f7349eefeb3744770c074e1b8b73f373fc1))

### Features

* icon-only actions in lists, multiline position description ([0a367b2](https://github.com/enke-dev/e-rng/commit/0a367b2444b33d392729938c5a394727ecbc6c72))
* invoice list and detail, position templates, sortable positions ([6387f6e](https://github.com/enke-dev/e-rng/commit/6387f6e3c620245d8554ad01febcd8f3cbddb25d))
* live preview of the invoice from the real template ([add23a8](https://github.com/enke-dev/e-rng/commit/add23a8690347f66c38bb60499e6f43dba0d8181))
* mark exported invoices as paid, filter the list by status, year and customer ([de55e62](https://github.com/enke-dev/e-rng/commit/de55e629f60bd56490d5cc38c6efe22561ceb98e))
* own invoice layouts, default in the settings and optional per invoice ([b651187](https://github.com/enke-dev/e-rng/commit/b65118794df7a81c61280fcc2aa9e8ba7dcbbeb6))

# [0.1.0](https://github.com/enke-dev/e-rng/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

* derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/e-rng/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)

# [0.1.0](https://github.com/enke-dev/e-rng/compare/v0.0.1...v0.1.0) (2026-10-17)

### Features

- derive the app icon from the icon composer document ([a406828](https://github.com/enke-dev/e-rng/commit/a406828651c041c13e85e24bbfad31e024acd038))

## 0.0.1 (2026-10-17)
