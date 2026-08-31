# format198x-atari-8bit-xex

Dependency-free parsing for Atari 8-bit segmented executable (`.xex`, also
seen as `.com` and `.obx`) files.

The parser preserves segment order and borrows each payload from the input.
Execution policy—copying segments into a machine, calling `INITAD`, and jumping
through `RUNAD`—belongs to the consumer.
