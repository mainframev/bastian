# Changelog

All notable changes to this project will be documented in this file.

## [0.4.1] - 2026-10-21

### Fixes

- *(search)* Return to where the search started on cancel by @mainframev in [#90](https://github.com/mainframev/bastian/pull/90) ([b5d6f88](https://github.com/mainframev/bastian/commit/b5d6f884dc055c30c69d538a0d66c4b2e8e5f6b0))
- *(search)* Find phrases that a line wrap splits by @mainframev in [#91](https://github.com/mainframev/bastian/pull/91) ([3020a8d](https://github.com/mainframev/bastian/commit/3020a8de8de73281cdf5eaad6beb2123c6e3952d))
- *(toc)* Jump to the numbered heading for any other number by @mainframev in [#92](https://github.com/mainframev/bastian/pull/92) ([786be2c](https://github.com/mainframev/bastian/commit/786be2c9b1ac340be8c3319cca5e19df7281c49d))
## [0.4.0] - 2026-10-17

### Features

- *(reader)* Drill into a ToC section with its number keys by @mainframev in [#52](https://github.com/mainframev/bastian/pull/52) ([7310208](https://github.com/mainframev/bastian/commit/7310208c5627af2a6e8955fc53f067df8bccf493))
- *(doc)* Bring MDX documents to par with Markdown by @mainframev in [#61](https://github.com/mainframev/bastian/pull/61) ([a83f066](https://github.com/mainframev/bastian/commit/a83f066d23066fc48782d6ed137fda01f6ab6cad))
- *(themes)* Paint the theme's page background by @mainframev in [#66](https://github.com/mainframev/bastian/pull/66) ([5355c6e](https://github.com/mainframev/bastian/commit/5355c6ea1bc770bd72cf145ba8c313170e8698aa))
- *(reader)* Shade the link under the mouse pointer by @mainframev in [#73](https://github.com/mainframev/bastian/pull/73) ([bccbad8](https://github.com/mainframev/bastian/commit/bccbad87212225b504e0ae36534e71600a0f7126))
- *(cli)* Read piped input when no path is given by @mainframev in [#75](https://github.com/mainframev/bastian/pull/75) ([f5db9a4](https://github.com/mainframev/bastian/commit/f5db9a42ff3d5f135d43c0aca6949d19a71cf106))
- *(config)* Turn the H history list off with history = false by @mainframev in [#76](https://github.com/mainframev/bastian/pull/76) ([3693007](https://github.com/mainframev/bastian/commit/36930073dcb8bb22bd775e8377c66a605ff89640))
- *(doc)* Style inline HTML formatting tags and ==mark== by @mainframev in [#78](https://github.com/mainframev/bastian/pull/78) ([c8fc010](https://github.com/mainframev/bastian/commit/c8fc0100f3466afeee9eb1d0750660928be4e35b))
- *(doc)* Show front matter as a key/value table by @mainframev in [#79](https://github.com/mainframev/bastian/pull/79) ([e1ba398](https://github.com/mainframev/bastian/commit/e1ba3981fe691926d5b76af18681185851d96baa))
- *(doc)* Render tex math as unicode text by @mainframev in [#80](https://github.com/mainframev/bastian/pull/80) ([33e2b62](https://github.com/mainframev/bastian/commit/33e2b621608ab0aa1c5b7816846c501d7a793c90))
- *(config)* Capture the mouse by default and set the wheel step by @mainframev in [#82](https://github.com/mainframev/bastian/pull/82) ([9fefa50](https://github.com/mainframev/bastian/commit/9fefa507d8882db8b97e846826a2aa75b8eaf7f9))

### Fixes

- *(doc)* Join MDX soft breaks and resolve reference links by @mainframev in [#58](https://github.com/mainframev/bastian/pull/58) ([6d70dff](https://github.com/mainframev/bastian/commit/6d70dffb9eb5caf2635cf882d6c739908befc950))
- *(reader)* Step through headings with j/k while the ToC is open by @mainframev in [#67](https://github.com/mainframev/bastian/pull/67) ([52bc19e](https://github.com/mainframev/bastian/commit/52bc19ee907568864fb94bec7c524eb6dfa24bc4))
- *(reader)* Walk a ToC section when its number is pressed again by @mainframev in [#69](https://github.com/mainframev/bastian/pull/69) ([48ec7b0](https://github.com/mainframev/bastian/commit/48ec7b0e4d2f576ea08cf4710668a888d95f2838))
- Tighter line-number gutter and hidden HTML comments by @mainframev in [#71](https://github.com/mainframev/bastian/pull/71) ([df3f0ee](https://github.com/mainframev/bastian/commit/df3f0ee006872355cc221a45632e6a97616011cb))
- Stop panics and silent failures found by a robustness audit by @mainframev in [#72](https://github.com/mainframev/bastian/pull/72) ([d53033c](https://github.com/mainframev/bastian/commit/d53033c6c196ba443f7512af4f34b8c8bfd0adbc))
- *(ui)* Build the link hover against link_rect returning an Option by @mainframev in [#74](https://github.com/mainframev/bastian/pull/74) ([3ec5c69](https://github.com/mainframev/bastian/commit/3ec5c6929e3a2520e85ecd783a6260c5835cccdf))
- *(browser)* Open the selected file with one Enter from the filter by @mainframev in [#77](https://github.com/mainframev/bastian/pull/77) ([302a63b](https://github.com/mainframev/bastian/commit/302a63ba3b1021510dbf25f73baf00fd658f7950))
## [0.3.0] - 2026-10-15

### Features

- *(reader)* Fold HTML <details> blocks by @mainframev in [#25](https://github.com/mainframev/bastian/pull/25) ([acd0627](https://github.com/mainframev/bastian/commit/acd06279da80cb3ab24a68206782419ecdbbe4bd))
- *(doc)* Follow [[wiki links]] by @mainframev in [#26](https://github.com/mainframev/bastian/pull/26) ([f5d09cf](https://github.com/mainframev/bastian/commit/f5d09cfeb3f66f7af511a5431b40f8a8f0fc9949))
- *(themes)* Add terminal, neutral and high-contrast themes by @mainframev in [#30](https://github.com/mainframev/bastian/pull/30) ([2ce881d](https://github.com/mainframev/bastian/commit/2ce881d700c57c053d85b9284ec4a025a81cd5df))
- Frame code blocks and highlight the focused one by @mainframev in [#33](https://github.com/mainframev/bastian/pull/33) ([25b61f7](https://github.com/mainframev/bastian/commit/25b61f7664458d8741399fda4bdf5b236b88817a))
- *(ui)* Group key help into sections with the version by @mainframev in [#31](https://github.com/mainframev/bastian/pull/31) ([6be3c71](https://github.com/mainframev/bastian/commit/6be3c71aaf177d7afaf5b8e6b3edcbec85d9fe5e))
- *(reader)* Show the file path with p by @mainframev in [#32](https://github.com/mainframev/bastian/pull/32) ([040289f](https://github.com/mainframev/bastian/commit/040289f0beaddb2d2969b2c2c2eea2f923b5ec46))
- *(reader)* Table of contents as a left pane by @mainframev in [#34](https://github.com/mainframev/bastian/pull/34) ([2933031](https://github.com/mainframev/bastian/commit/2933031300aebbf3c3e9b418ec9d9185ce0bad5d))
- *(render)* Draw mermaid diagrams behind a mermaid feature by @mainframev in [#36](https://github.com/mainframev/bastian/pull/36) ([261b5bd](https://github.com/mainframev/bastian/commit/261b5bdefcccd878ab143d31ea45bac181147cc2))
- *(ui)* Show the theme picker as a right pane by @mainframev in [#37](https://github.com/mainframev/bastian/pull/37) ([465e2ab](https://github.com/mainframev/bastian/commit/465e2abb2603673514e8381e7c7287b8fb7f43cf))
- *(browser)* List the main keys in the status bar by @mainframev in [#43](https://github.com/mainframev/bastian/pull/43) ([5eae9d5](https://github.com/mainframev/bastian/commit/5eae9d532e84b4df138da3571b671094949b594b))
- *(reader)* Page and filter the links list by @mainframev in [#47](https://github.com/mainframev/bastian/pull/47) ([912f0b1](https://github.com/mainframev/bastian/commit/912f0b199738cc77a5a819ad2d9c5dda91746aac))
- *(reader)* Go to a line with : by @mainframev in [#45](https://github.com/mainframev/bastian/pull/45) ([a3786e3](https://github.com/mainframev/bastian/commit/a3786e326a910855304fd8eb1e4afe52bcdd038c))
- *(reader)* List this session's files with H by @mainframev in [#46](https://github.com/mainframev/bastian/pull/46) ([41b0ee8](https://github.com/mainframev/bastian/commit/41b0ee8ab791c050775d5f1791e606ed75078f54))
- *(render)* Style headings, bullets, inline code and quotes by element by @mainframev in [#49](https://github.com/mainframev/bastian/pull/49) ([e33e92f](https://github.com/mainframev/bastian/commit/e33e92f4ce4d664dec731df4ed3d5568320c0dd3))
- *(reader)* Highlight the line : goes to by @mainframev in [#51](https://github.com/mainframev/bastian/pull/51) ([10cb3d1](https://github.com/mainframev/bastian/commit/10cb3d1a622c35d7a293160e4888b74f24c7e5b8))

### Fixes

- Proper syntax highlighting for common languages and token kinds by @mainframev in [#24](https://github.com/mainframev/bastian/pull/24) ([341b360](https://github.com/mainframev/bastian/commit/341b360e8a05716005b4f45f7419716e3325fbcb))
- *(ui)* Center popups on the document, not the whole screen by @mainframev in [#38](https://github.com/mainframev/bastian/pull/38) ([14425b2](https://github.com/mainframev/bastian/commit/14425b2e412b5367b01e69129cc184cfbc738b5d))
- *(ui)* Fit theme swatches to the pane on each theme's surface by @mainframev in [#41](https://github.com/mainframev/bastian/pull/41) ([3d4481a](https://github.com/mainframev/bastian/commit/3d4481a942a7c5d38da15d30525869d041f19feb))
- *(ui)* Drop the ? row from the key help by @mainframev in [#48](https://github.com/mainframev/bastian/pull/48) ([26c4fd5](https://github.com/mainframev/bastian/commit/26c4fd5a0462bd18db6760c1026453717b0b1caf))
- *(reader)* Keep hyperlinks from printing over popups by @mainframev in [#42](https://github.com/mainframev/bastian/pull/42) ([293e253](https://github.com/mainframev/bastian/commit/293e2530937816f630fc2e1a83d1d463ed679211))
- *(reader)* Mark the heading a ToC jump aimed at near the end by @mainframev in [#50](https://github.com/mainframev/bastian/pull/50) ([710596c](https://github.com/mainframev/bastian/commit/710596c9d85fbffd81b1cddf6317745867a6ee02))
## [0.2.0] - 2026-10-12

### Features

- *(render)* GitHub alerts, footnotes and task checkboxes by @mainframev in [#10](https://github.com/mainframev/bastian/pull/10) ([80b9f72](https://github.com/mainframev/bastian/commit/80b9f72a5d00d832fbd6acad9e4d79628b3a6ec4))
- *(cli)* [**breaking**] Drop the man command, take on|off for --images by @mainframev in [#15](https://github.com/mainframev/bastian/pull/15) ([ae6732c](https://github.com/mainframev/bastian/commit/ae6732cbe0cf857e7b4e207b09bc4d58fab7f3e5))
- *(reader)* Reopen files where they were last read by @mainframev in [#12](https://github.com/mainframev/bastian/pull/12) ([3d29255](https://github.com/mainframev/bastian/commit/3d292555fc5558fb8ee174a8ca81f81897514af7))
- *(render)* Break CJK text between characters by @mainframev in [#11](https://github.com/mainframev/bastian/pull/11) ([a69be4e](https://github.com/mainframev/bastian/commit/a69be4e4264c5744039f276edb14a45f358b6271))
- *(reader)* Step through links in place with s/S by @mainframev in [#13](https://github.com/mainframev/bastian/pull/13) ([29b4b25](https://github.com/mainframev/bastian/commit/29b4b256388f9a186c0005956ba18b09232f0997))

### Fixes

- *(doc)* Cap container nesting depth by @mainframev in [#16](https://github.com/mainframev/bastian/pull/16) ([8d983cd](https://github.com/mainframev/bastian/commit/8d983cd6d25e2ecb9f6b994211509e317cae73fb))
- *(remote)* Cap downloaded bytes, not compressed bytes by @mainframev in [#17](https://github.com/mainframev/bastian/pull/17) ([3b5e444](https://github.com/mainframev/bastian/commit/3b5e444835ec2eadbbbfecff8a8fea670f8e23a0))
- *(reader)* Confirm before opening non-web links by @mainframev in [#18](https://github.com/mainframev/bastian/pull/18) ([7895984](https://github.com/mainframev/bastian/commit/7895984479ba3a58c1516348a110065208ccaf16))
- *(exec)* Bound block output and kill blocks that ignore SIGTERM by @mainframev in [#19](https://github.com/mainframev/bastian/pull/19) ([e14728c](https://github.com/mainframev/bastian/commit/e14728c04417ffd355cc940f7acc8fd8d01552ac))
- *(term)* Keep terminal queries away from pagers and background jobs by @mainframev in [#20](https://github.com/mainframev/bastian/pull/20) ([b7b1e5f](https://github.com/mainframev/bastian/commit/b7b1e5fccbd036a977b5210320af8979fb49e1ab))
## [0.1.0] - 2026-10-12

### Features

- Initial markdown reader by @mainframev ([48b6f9d](https://github.com/mainframev/bastian/commit/48b6f9d9c4d4784dd6ff449db080f36a5455a335))
- Add file browser and in-document search by @mainframev ([74b00f9](https://github.com/mainframev/bastian/commit/74b00f92d0722a84a063e02a301fea10f3c9cbe1))
- Run fenced code blocks inline by @mainframev ([8d6d6ce](https://github.com/mainframev/bastian/commit/8d6d6cea2ae561aa188a741ee2494024c9664634))
- Add config file, link picker, copy, reload and help by @mainframev ([77e961a](https://github.com/mainframev/bastian/commit/77e961a44a1b99522c0ae3fde4a1f4581f93ae16))
- *(ui)* Logo header, top find line and file dates by @mainframev ([1b6fd07](https://github.com/mainframev/bastian/commit/1b6fd07dadf88b7575658158e6fe8c1b436b7682))
- *(browser)* Page the file list with ctrl-d/u and ctrl-f/b by @mainframev ([ae1e9cd](https://github.com/mainframev/bastian/commit/ae1e9cd376fb57e97e59be5777bccaf1e2eafc67))
- Open files in the editor and show a "? more" hint by @mainframev ([bda69dc](https://github.com/mainframev/bastian/commit/bda69dc11cab2b02a33d19b9ad65bb6a5739eb1c))
- *(browser)* Turn the file list by pages with a page indicator by @mainframev ([068b112](https://github.com/mainframev/bastian/commit/068b11281db91e3887a85e93591a00075585db8f))
- *(browser)* Turn pages with h/l and arrow keys by @mainframev ([68bfd27](https://github.com/mainframev/bastian/commit/68bfd27df7590824e6b7484118095fe4fa21107e))
- *(browser)* Navigate filter results without pressing Enter by @mainframev ([b0ca79c](https://github.com/mainframev/bastian/commit/b0ca79c926c90e230764a134a2777483a7b1e490))
- *(reader)* Optional line numbers by @mainframev ([37bf282](https://github.com/mainframev/bastian/commit/37bf2825d747c4ebb57b17658c30335071fbe298))
- *(browser)* Move page indicator below the list, split path and date by @mainframev ([6580211](https://github.com/mainframev/bastian/commit/6580211e13cd10c091514599fbd881adb4debf55))
- *(browser)* Gradient big-text logo in the header by @mainframev ([34fa31c](https://github.com/mainframev/bastian/commit/34fa31c0fdeb74ec0d823e1eaebc0cbc652465e3))
- Render MDX files and hide front matter by @mainframev ([04a7483](https://github.com/mainframev/bastian/commit/04a7483d5d73b2bdaacb3cf9e216c0cd8d0b8c27))
- *(output)* Colored print and pager output by @mainframev ([fe10a69](https://github.com/mainframev/bastian/commit/fe10a69fa36b3cff4c31ad4e6bdd957bb61057d9))
- *(theme)* Pick light or dark from the terminal background by @mainframev ([d0f394b](https://github.com/mainframev/bastian/commit/d0f394bbfacfd0256298a8534f67be67a21b118d))
- *(browser)* List hidden files on request and read BOM files by @mainframev ([8a889ed](https://github.com/mainframev/bastian/commit/8a889edc489a56a59082dcf5519adb933a2b2d85))
- *(reader)* Show source files as highlighted code by @mainframev ([72d3cc8](https://github.com/mainframev/bastian/commit/72d3cc8037486416055896f4a34ddc281b34eb7b))
- *(reader)* Copy the document, edit at the current line, suspend by @mainframev ([c6ecdc2](https://github.com/mainframev/bastian/commit/c6ecdc2b00fdf6bb7868ebbc155272c22d220c8c))
- *(config)* Choose the config file, inspect and edit settings by @mainframev ([728d62b](https://github.com/mainframev/bastian/commit/728d62b19c8600a5d19fb4dca234afdb0907aa55))
- Read Markdown from URLs and GitHub/GitLab repositories by @mainframev ([94eb2fc](https://github.com/mainframev/bastian/commit/94eb2fc87395a3acf82d8bc172d6836a3a0ac3fc))
- *(cli)* Shell completions, man page, mouse wheel by @mainframev ([467075a](https://github.com/mainframev/bastian/commit/467075aed227d76f866f01d8343c24067761af31))
- *(render)* Wrap, align and style table cells by @mainframev ([0fbf94f](https://github.com/mainframev/bastian/commit/0fbf94f04b9255b739c2788407acbe11983be266))
- *(browser)* Omit path when it repeats the name by @mainframev ([de957ca](https://github.com/mainframev/bastian/commit/de957ca4083831ddc4c7d9ade159f0c60735a4ff))
- *(theme)* Built-in themes with palette-derived code colors by @mainframev ([3a67969](https://github.com/mainframev/bastian/commit/3a67969f531514746042fd21fe413e987d728ede))
- *(theme)* User theme files and a themes command by @mainframev ([eb9c266](https://github.com/mainframev/bastian/commit/eb9c2666d2965163554c7b9cc42e8e8998e71829))
- *(theme)* Interactive theme picker by @mainframev ([5678c5c](https://github.com/mainframev/bastian/commit/5678c5c1123bbf4cbbfad646126792725f3df164))
- *(browser)* Option to hide the logo by @mainframev ([de89b4f](https://github.com/mainframev/bastian/commit/de89b4f278d5dc477ed1ffae5053076aa02b1fe7))
- *(input)* Editable search and filter input by @mainframev ([6d01d0f](https://github.com/mainframev/bastian/commit/6d01d0f7098d196b7c101222c0596f0dca5f808e))
- *(ui)* Scrollbars for the reader and the browser by @mainframev ([a267bff](https://github.com/mainframev/bastian/commit/a267bffaf53e0c7047ac7cb1ab356421bc4e45af))
- *(reader)* Clickable links via OSC 8 by @mainframev ([871d87f](https://github.com/mainframev/bastian/commit/871d87f73ed996ac35a1693bb5166f091c7edd3d))
- *(reader)* Heading outline with tui-tree-widget by @mainframev ([305ad52](https://github.com/mainframev/bastian/commit/305ad526cf541258c3eb1d17d51249d619410558))
- *(reader)* Show images with ratatui-image by @mainframev ([0ddba9b](https://github.com/mainframev/bastian/commit/0ddba9b797b80e2d841ef0aaddb82a0d10d16185))
- *(reader)* Toggle images with `i` by @mainframev ([f44b1eb](https://github.com/mainframev/bastian/commit/f44b1eb5402488a6083c5ec9f0cd8c5a05835dc7))
- *(browser)* Selector style and compact layout options by @mainframev ([6a7b6ad](https://github.com/mainframev/bastian/commit/6a7b6ad1999fe4197bde71ca2502f5022a0f88a6))
- *(browser)* Sort, date, extension and ignore options by @mainframev ([44068bf](https://github.com/mainframev/bastian/commit/44068bfcd76ab7d8865f2f5121590e32a2d8c160))
- *(reader)* Image height, tab width, hyperlink, scrollbar and editor options by @mainframev ([3cc7c89](https://github.com/mainframev/bastian/commit/3cc7c892e4350c8607be517cb9b0db1c9a625763))

### Fixes

- *(browser)* Add a blank row between the page indicator and status bar by @mainframev ([86e6673](https://github.com/mainframev/bastian/commit/86e6673a1d29060bc3eb97bc796713d0b88ed96c))
- *(theme)* Wait for the terminal's reply with select, not poll by @mainframev ([97d90d2](https://github.com/mainframev/bastian/commit/97d90d28914ff436026b16329f55f5b19b8927ad))
- *(browser)* Keep a single gap after entries without a path by @mainframev ([1717190](https://github.com/mainframev/bastian/commit/17171907539f258ab51014b90341d9b28a23cc09))
- *(browser)* Keep hidden paths out unless --all by @mainframev ([f75f7ea](https://github.com/mainframev/bastian/commit/f75f7ea308da7ab4fa79af7323a0e87d64f26149))
- *(remote)* Read GitHub and GitLab file pages as raw files by @mainframev ([8de5985](https://github.com/mainframev/bastian/commit/8de5985fae1beefd2aa7c194fb504b5b0d93c5a7))
- *(cli)* Show the same help for -h and --help by @mainframev ([6481b32](https://github.com/mainframev/bastian/commit/6481b327e24eceb48fafec59eeb963b6123301bb))
- *(reader)* Keep a blank row above and below the document by @mainframev ([88975db](https://github.com/mainframev/bastian/commit/88975db46cdbbe588936620885ef497e5fc62deb))
- *(images)* Keep keys working and show partly scrolled pictures by @mainframev ([ccf15c0](https://github.com/mainframev/bastian/commit/ccf15c01bedb3f658191a6a23ff0730ac334dc9a))
- *(images)* Ask the terminal for graphics support at startup by @mainframev ([31edf29](https://github.com/mainframev/bastian/commit/31edf29c79e3a0e35498498d7ba97f945e56e84f))
- *(reader)* Leave a gap between line numbers and text by @mainframev ([56ac10c](https://github.com/mainframev/bastian/commit/56ac10c8af41fcdc646d2b1bc38c55b4bbad0917))
- *(images)* Leave GIFs and SVGs as alt text without fetching them by @mainframev ([959fbfe](https://github.com/mainframev/bastian/commit/959fbfedb5913cab2d8f9a0796d844e7467f6ba7))
- Pass clippy on Rust 1.99 by @mainframev in [#3](https://github.com/mainframev/bastian/pull/3) ([ca74275](https://github.com/mainframev/bastian/commit/ca7427588ad0c4f4dd09cee184a9fab46a519c50))
- *(render)* Strip control characters from document text by @mainframev ([b2433b6](https://github.com/mainframev/bastian/commit/b2433b6dc8237a14e2991da2a9318a0eec9e0717))
- *(reader)* Drop pending run confirmation when code blocks change on reload by @mainframev ([cd1056f](https://github.com/mainframev/bastian/commit/cd1056f0a45451fa65cadc60f34bcacd2d2d0012))
- *(browser)* Measure shortened paths by display width by @mainframev ([3da4045](https://github.com/mainframev/bastian/commit/3da40452f7ca91c501410d1d65d382545d79c77e))
- *(images)* Read only regular files and cap decode size by @mainframev ([e7eace7](https://github.com/mainframev/bastian/commit/e7eace7acabed3485ba911b3f981863c94b865f8))
- *(doc)* Reject overflowing timeouts and strip attrs from MDX code language by @mainframev ([40e7448](https://github.com/mainframev/bastian/commit/40e744851925264d2f3d65c1bf51f0f569c1c040))
- *(app)* Turn mouse capture off on panic and failed setup by @mainframev ([03b1dc8](https://github.com/mainframev/bastian/commit/03b1dc898359ac5e8c0965f1e212dc6422d8380d))
- *(term)* Discard a terminal reply cut off by the query timeout by @mainframev ([a95ae6a](https://github.com/mainframev/bastian/commit/a95ae6a0624c118bf9e949658e9ff57ef3f4da70))
- *(output)* Keep piped output plain when the pager is switched on by @mainframev ([1f73898](https://github.com/mainframev/bastian/commit/1f738981a8f6f0b4ac51896c03d6d744f13d9487))
- *(reader)* Jump to the heading block itself when following an anchor by @mainframev ([2cff04e](https://github.com/mainframev/bastian/commit/2cff04e531c8061239576a0291e16a22a8ad44d7))
<!-- generated by git-cliff -->
