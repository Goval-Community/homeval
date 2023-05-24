<center>
    <img width="70%" src="./media/full-logo.svg">
    <br>
    <a href="https://www.gnu.org/licenses/agpl-3.0"><img alt="License: AGPL-3.0-only" src="https://img.shields.io/badge/License-AGPL--3.0--only-9958f7.svg">
    </a>
    <a href="https://github.com/potentialstyx/issues/1"><img alt="Services implemented: 4" src ="https://img.shields.io/badge/services%20implemented-4-9958f7"></a>
    <!--
    uncomment when public repo
        <a href="https://github.com/potentialstyx/homeval/pulls"><img alt="current # of open pull requests" src="https://img.shields.io/github/issues-pr/potentialstyx/homeval?color=9958f7"></a>
    -->
    <hr><br>
    <p>Homeval is a custom server implementation of <a href="https://govaldocs.pages.dev">goval</a>, replits evaluation protocol.</p>
</center>

# License
Homeval is licensed under GNU AGPL-3.0-only  

### Restrictions on running homeval on replit
Unfortunately due to replit's TOS, AGPL programs cannot be run in public repls. Though, private repls are fine, as long as you still fulfill the terms of the license.  
This is due to <a href="https://docs.replit.com/legal-and-security-info/licensing-info#public-repls-and-teams">all public repls being licensed under MIT</a>, and GPL code cannot be included in a MIT licensed project.

# Running homeval

Homeval can be built into a binary with `cargo build --release` the binary will then end up in `target/release/homeval` (make sure to set `RUST_LOG=INFO` when running this binary or you won't get any logs). To compile and run a debug build use `cargo run`.

When running a built binary there is currently one major requirement: you need to have a copy of the `services` directory wherever you invoke homeval. A fix for this is currently a WIP.

# Implementing a service

Make a new file in `services/` name it with the format `<service name>.js` then see existing services and `src/runtime.js` for the interface you need to provide. Docs focussed on implementing services are a WIP.

# Supported targets

All linux distros with an up to date enough GLIBC should work. The only distro official tested however is arch linux.

Using musl libc, Windows, or MacOS is not officially supported right now. You might encounter roadblocks attempting to compile targeting any of these targets.

Official windows support is a WIP.