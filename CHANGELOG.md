# Changelog

All notable changes to this project will be documented in this file.

## [0.3.0] - 2026-10-15

### Features

- *(reader)* Fold HTML <details> blocks  by @mainframev in [#25](https://github.com/mainframev/bastian/pull/25) ([3c03c11](https://github.com/mainframev/bastian/commit/3c03c11f5d112c919f08c50951c613d8e16b2bcc))
- *(doc)* Follow [[wiki links]]  by @mainframev in [#26](https://github.com/mainframev/bastian/pull/26) ([72724f8](https://github.com/mainframev/bastian/commit/72724f837823417b608a91db6710e51d6001a954))
- *(themes)* Add terminal, neutral and high-contrast themes  by @mainframev in [#30](https://github.com/mainframev/bastian/pull/30) ([6fc4bcd](https://github.com/mainframev/bastian/commit/6fc4bcdcf37c4f6cecd9b90e0feb6f301250c000))
- Frame code blocks and highlight the focused one  by @mainframev in [#33](https://github.com/mainframev/bastian/pull/33) ([4e1a51c](https://github.com/mainframev/bastian/commit/4e1a51c16e077a665e06499af0f2941a92e8e6af))
- *(ui)* Group key help into sections with the version  by @mainframev in [#31](https://github.com/mainframev/bastian/pull/31) ([dabf4a3](https://github.com/mainframev/bastian/commit/dabf4a3c6907d5b5dd1b853e3149111632b40140))
- *(reader)* Show the file path with p  by @mainframev in [#32](https://github.com/mainframev/bastian/pull/32) ([a172638](https://github.com/mainframev/bastian/commit/a1726388aae6013192619cf600e4ecb46faa51b9))
- *(reader)* Table of contents as a left pane  by @mainframev in [#34](https://github.com/mainframev/bastian/pull/34) ([bdaf4eb](https://github.com/mainframev/bastian/commit/bdaf4eb41cb2ea55600294aea544c4eb7bb86907))
- *(render)* Draw mermaid diagrams behind a mermaid feature  by @mainframev in [#36](https://github.com/mainframev/bastian/pull/36) ([4cdbc14](https://github.com/mainframev/bastian/commit/4cdbc14dabbac1ddfb93d29c313edbfed9e4521f))
- *(ui)* Show the theme picker as a right pane  by @mainframev in [#37](https://github.com/mainframev/bastian/pull/37) ([67bf5e6](https://github.com/mainframev/bastian/commit/67bf5e6ab5c16c24e1889491681ef701c4aeeb58))
- *(browser)* List the main keys in the status bar  by @mainframev in [#43](https://github.com/mainframev/bastian/pull/43) ([94bbfbd](https://github.com/mainframev/bastian/commit/94bbfbd5aee41c05eef1ba2136cf706d67cd241c))
- *(reader)* Page and filter the links list  by @mainframev in [#47](https://github.com/mainframev/bastian/pull/47) ([e31de59](https://github.com/mainframev/bastian/commit/e31de59d652cbb4e741b3ac5fc387ae36f6041be))
- *(reader)* Go to a line with :  by @mainframev in [#45](https://github.com/mainframev/bastian/pull/45) ([91b9257](https://github.com/mainframev/bastian/commit/91b92579f7b1d20a0cd648ddb4a936a9feae021f))
- *(reader)* List this session's files with H  by @mainframev in [#46](https://github.com/mainframev/bastian/pull/46) ([556c148](https://github.com/mainframev/bastian/commit/556c1484832123d16d49b1904b4f2f4e44aa9e1b))
- *(render)* Style headings, bullets, inline code and quotes by element  by @mainframev in [#49](https://github.com/mainframev/bastian/pull/49) ([2b2fbc1](https://github.com/mainframev/bastian/commit/2b2fbc1e3bbec33298e24c14fd3f152659059ed9))

### Fixes

- Proper syntax highlighting for common languages and token kinds  by @mainframev in [#24](https://github.com/mainframev/bastian/pull/24) ([5f59aa8](https://github.com/mainframev/bastian/commit/5f59aa8f18e5cc342ecd25f12eedadbeaa55d91a))
- *(ui)* Center popups on the document, not the whole screen  by @mainframev in [#38](https://github.com/mainframev/bastian/pull/38) ([c780011](https://github.com/mainframev/bastian/commit/c780011d9f25cb1112f5321542e0dcbcd090a286))
- *(ui)* Fit theme swatches to the pane on each theme's surface  by @mainframev in [#41](https://github.com/mainframev/bastian/pull/41) ([3d16658](https://github.com/mainframev/bastian/commit/3d1665875bb3dc28a0e3ef44e083ef3983536b2b))
- *(ui)* Drop the ? row from the key help  by @mainframev in [#48](https://github.com/mainframev/bastian/pull/48) ([5e2159e](https://github.com/mainframev/bastian/commit/5e2159e96285359b88de48f1b141ad319af001f8))
- *(reader)* Keep hyperlinks from printing over popups  by @mainframev in [#42](https://github.com/mainframev/bastian/pull/42) ([db744c8](https://github.com/mainframev/bastian/commit/db744c871a87da9ea567efbd88766aebfd077522))
- *(reader)* Mark the heading a ToC jump aimed at near the end  by @mainframev in [#50](https://github.com/mainframev/bastian/pull/50) ([4d1e5aa](https://github.com/mainframev/bastian/commit/4d1e5aabdfd482810eb0f0e217c34a66d247e36a))
## [0.2.0] - 2026-10-12

### Features

- *(render)* GitHub alerts, footnotes and task checkboxes  by @mainframev in [#10](https://github.com/mainframev/bastian/pull/10) ([3905cf0](https://github.com/mainframev/bastian/commit/3905cf0f782a0821bb27945d8d359213dc2d3caa))
- *(cli)* [**breaking**] Drop the man command, take on|off for --images  by @mainframev in [#15](https://github.com/mainframev/bastian/pull/15) ([b96f688](https://github.com/mainframev/bastian/commit/b96f688a74870b09a4dbd42e687ba21d252560e0))
- *(reader)* Reopen files where they were last read  by @mainframev in [#12](https://github.com/mainframev/bastian/pull/12) ([3950c23](https://github.com/mainframev/bastian/commit/3950c2386b7d66c356f6edd73b99711a62c3527f))
- *(render)* Break CJK text between characters  by @mainframev in [#11](https://github.com/mainframev/bastian/pull/11) ([229d953](https://github.com/mainframev/bastian/commit/229d953d3ee2348b2a8333ddb85631ec5a6c431a))
- *(reader)* Step through links in place with s/S  by @mainframev in [#13](https://github.com/mainframev/bastian/pull/13) ([7b48504](https://github.com/mainframev/bastian/commit/7b485043438e694282c96183ed226318e8734e66))

### Fixes

- *(doc)* Cap container nesting depth  by @mainframev in [#16](https://github.com/mainframev/bastian/pull/16) ([8f7bfbb](https://github.com/mainframev/bastian/commit/8f7bfbbaa650432bebb5ebbabc896954bd8c4e37))
- *(remote)* Cap downloaded bytes, not compressed bytes  by @mainframev in [#17](https://github.com/mainframev/bastian/pull/17) ([4aaf053](https://github.com/mainframev/bastian/commit/4aaf053f67bf5e0d0b63cf783050cd506db3f888))
- *(reader)* Confirm before opening non-web links  by @mainframev in [#18](https://github.com/mainframev/bastian/pull/18) ([635debb](https://github.com/mainframev/bastian/commit/635debb1911d9f1a41a6c27fb367a23496adf57a))
- *(exec)* Bound block output and kill blocks that ignore SIGTERM  by @mainframev in [#19](https://github.com/mainframev/bastian/pull/19) ([479940d](https://github.com/mainframev/bastian/commit/479940dbea1cf20d521581bf40e9e69a70592439))
- *(term)* Keep terminal queries away from pagers and background jobs  by @mainframev in [#20](https://github.com/mainframev/bastian/pull/20) ([4251949](https://github.com/mainframev/bastian/commit/4251949c142dc69f2804c4c7db61efb9184885e3))
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
- *(images)* Leave GIFs and SVGs as alt text without fetching them by @mainframev ([2f51889](https://github.com/mainframev/bastian/commit/2f51889df478c3038b9486c7b714aac6e3d90e7d))
- Pass clippy on Rust 1.99  by @mainframev in [#3](https://github.com/mainframev/bastian/pull/3) ([bc8b5ad](https://github.com/mainframev/bastian/commit/bc8b5ade6859e97a2a86aba19decdcbecba25b08))
- *(render)* Strip control characters from document text by @mainframev ([a872326](https://github.com/mainframev/bastian/commit/a872326f584e1f559c2ccf1b6f0dbd0132625524))
- *(reader)* Drop pending run confirmation when code blocks change on reload by @mainframev ([1e09a7e](https://github.com/mainframev/bastian/commit/1e09a7e893899965127a876e74f685e6b74c0735))
- *(browser)* Measure shortened paths by display width by @mainframev ([276a581](https://github.com/mainframev/bastian/commit/276a581f0874368add426493d2c62efec669db93))
- *(images)* Read only regular files and cap decode size by @mainframev ([1d20902](https://github.com/mainframev/bastian/commit/1d20902797d502dab7fae654fccb8ecc346bba86))
- *(doc)* Reject overflowing timeouts and strip attrs from MDX code language by @mainframev ([557751b](https://github.com/mainframev/bastian/commit/557751b0bd92126b767ada3c0fa33cfc322e1684))
- *(app)* Turn mouse capture off on panic and failed setup by @mainframev ([af2757c](https://github.com/mainframev/bastian/commit/af2757cb36d7d20c731be20aee56b25763c511b4))
- *(term)* Discard a terminal reply cut off by the query timeout by @mainframev ([77d09aa](https://github.com/mainframev/bastian/commit/77d09aa9116bd7ea1522fe7dfbabc34b01e699ad))
- *(output)* Keep piped output plain when the pager is switched on by @mainframev ([cfceb24](https://github.com/mainframev/bastian/commit/cfceb24cec358f4ada4223c5317ade2aa0047060))
- *(reader)* Jump to the heading block itself when following an anchor by @mainframev in [#2](https://github.com/mainframev/bastian/pull/2) ([daaa864](https://github.com/mainframev/bastian/commit/daaa864af2fa8d0ed50bcd7f579b395fe4a21a38))
<!-- generated by git-cliff -->
