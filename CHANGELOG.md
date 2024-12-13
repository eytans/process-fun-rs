# Changelog

All notable changes to this project will be documented in this file.

## [0.1.2] - 2024-12-13

- Fixed a timing bug in child creation. The child now uses `stat` to find its system start time to ensure correct termination on drop.

## [0.1.1] - 2024-12-12

- Fixed a bug in dependency management inside the macro. Missing dependency exports from `process-fun` have been re-exported to make it work.