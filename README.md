<div align="center">
    <img width="70%" src="./media/full-logo.svg">
    <br>
    <a href="https://www.gnu.org/licenses/agpl-3.0"><img alt="License: AGPL-3.0-only" src="https://img.shields.io/badge/License-AGPL--3.0--only-9958f7">
    </a>
    <img alt="Services implemented: 14" src ="https://img.shields.io/badge/services%20implemented-14-9958f7">
    <hr><br>
    <p>Homeval is a custom server implementation of <a href="https://govaldocs.pages.dev">goval</a>, replits evaluation protocol.</p>
</div>

# License
Homeval is licensed under GNU AGPL-3.0-only  

### Restrictions on running homeval on replit
Unfortunately due to replit's TOS, AGPL programs cannot be run in public repls. Though, private repls are fine, as long as you still fulfill the terms of the license.  
This is due to <a href="https://docs.replit.com/legal-and-security-info/licensing-info#public-repls-and-teams">all public repls being licensed under MIT</a>, and GPL code cannot be included in a MIT licensed project.

# Running homeval

## Installation
1. Git clone the repository
2. Install required dependencies
    * If on macOS or linux: [Bun](https://bun.sh/) and [Git](https://git-scm.com/downloads)
    * If on windows: [Node.js](https://nodejs.org/en/download), [Yarn v1](https://classic.yarnpkg.com/lang/en/docs/install/#windows-stable) and [Git for Windows](https://gitforwindows.org/)
    * The [Protobuf Compiler](https://github.com/protocolbuffers/protobuf/releases)
        * If using a debian linux based distro just run: `sudo apt install protobuf-compiler`
    * [Rustup](https://rustup.rs/)
    * And finally, [Ripgrep](https://github.com/BurntSushi/ripgrep#installation).

## ⚠️ Notice for windows users
On windows `cargo run` as well as invoking the built binary must happen inside the [Git Bash](https://gitforwindows.org/) shell.

Console and shell will not work, this will be fixed later. You should submit a bug report for other broken features.

## Building

Homeval can be built into a binary with `cargo build --release` the binary will then end up in `target/release/homeval` (make sure to set `RUST_LOG=INFO` when running this binary or you won't get any logs). 

## Running
To compile and run a debug build use `cargo run`.

# Implementing a service

Make a new file in `services/` name it with the format `<service name>.js` then see existing services and `src/runtime.js` for the interface you need to provide. Docs focussed on implementing services are a WIP.

> NOTE: The source code for services are compiled in to release builds, but loaded at runtime for debug builds.

# Supported targets


| Target  | Will Compile | Officially Supported | Feature Complete | Tested[^testing] |
| --- | --- | --- | --- | --- |
| Linux[^linux]  | ✅ | ✅ | ✅ | ✅[^linux-tests] |
| macOS[^macos]  | ✅ | ✅ | ✅ | ❎ | 
| Windows | ✅ | ✅ | ❎[^windows] | ❎ |


[^testing]: This marks if every release is officially tested for this target.

[^macos]: Please not that PotentialStyx (the main dev) does not have any machines that run macOS, so issues on macOS might take longer to fix.

[^linux]: The distro has to have an up to date GLIBC version, musl is not supported. 

[^linux-tests]: Currently, the only tested distribution is arch linux. Though all distros with an up to date GLIC *should* work.

[^windows]: Shell and Console support are currently unavailable on windows.

# TODO:

- [ ] Embedded repldb server
- [ ] Have windows builds feature complete
- [ ] Debugger support
- [ ] Audio channel support