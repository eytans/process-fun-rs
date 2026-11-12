# Changelog

All notable changes to this project will be documented in this file.

## [0.1.3] - 2026-11-12

- Child processes are now reaped (`waitpid`) on drop and on `abort()`, so they no longer linger as zombies (#1).
- Dropping or aborting a handle before `wait()`/`timeout()` now actually kills the child.
- A forked child now always exits, even if the function panics or an internal error occurs; previously it could unwind or return into the caller's code and keep running as a copy of the parent.
- Added `ProcessWrapper::pid()`.

## [0.1.2] - 2024-12-13

- Fixed a timing bug in child creation. The child now uses `stat` to find its system start time to ensure correct termination on drop.
- Turned off default debug feature.
- Cleanup and support type parameters

## [0.1.1] - 2024-12-12

- Fixed a bug in dependency management inside the macro. Missing dependency exports from `process-fun` have been re-exported to make it work.