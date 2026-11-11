# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

- Child processes are now reaped (`waitpid`) on drop and on `abort()`, so they no longer linger as zombies (#1).
- Dropping or aborting a handle before `wait()`/`timeout()` now actually kills the child.
- Handles inherited by a forked child (e.g. when a panic unwinds in the child) no longer kill the parent's other children.
- Added `ProcessWrapper::pid()`.

## [0.1.2] - 2024-12-13

- Fixed a timing bug in child creation. The child now uses `stat` to find its system start time to ensure correct termination on drop.
- Turned off default debug feature.
- Cleanup and support type parameters

## [0.1.1] - 2024-12-12

- Fixed a bug in dependency management inside the macro. Missing dependency exports from `process-fun` have been re-exported to make it work.