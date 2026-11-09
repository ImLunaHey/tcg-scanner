# Native scanner contract fixture

`sane.c` implements the small SANE 1 ABI used by the Rust adapter. It simulates a
duplex feeder and never accesses hardware. Compile it explicitly as development
tooling; neither the application nor its tests spawn a compiler or scanner helper.

macOS:

```sh
clang -std=c11 -Wall -Wextra -Werror -dynamiclib Tools/NativeDeviceTests/sane.c -o /tmp/scanner-sane-fixture.dylib
SCANNER_SANE_TEST_LIBRARY=/tmp/scanner-sane-fixture.dylib cargo test -p scanner-device --test sane_contract
```

Linux:

```sh
cc -std=c11 -D_POSIX_C_SOURCE=200809L -Wall -Wextra -Werror -shared -fPIC Tools/NativeDeviceTests/sane.c -o /tmp/scanner-sane-fixture.so
SCANNER_SANE_TEST_LIBRARY=/tmp/scanner-sane-fixture.so cargo test -p scanner-device --test sane_contract
```

Only select this trusted test library: the test expects its `fixture_*` exports.
Without the environment variable the optional native ABI test does not run its
scenarios. Ordinary frame/driver tests still run without it.

The scenarios exercise discovery, read-only inspection, acknowledged settings
before the first feed, padded/chunked RGB output, ordered duplex frames, restored
settings after success and inexact writes, no feed after persistence failure,
read/close ordering under cancellation, and cleanup after a caller disconnects.
These simulations do not establish hardware compatibility.
