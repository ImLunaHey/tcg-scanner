# Scanner platform state

Native private local files and a process-lifetime hardware owner lock. This crate
has no UI, database, network client or helper-process dependency.

`PrivateDirectory` pins a directory capability and rejects symbolic links while
walking its components. Relative file operations accept one filename, reject
links and validate ownership/permissions on the opened native handle. Credential
creation is exclusive, synchronized and never replaces existing credentials;
loading is bounded to a 64-character lowercase hexadecimal token plus newline.
Token generation remains the caller's responsibility.

Unix state must belong to the effective user with no group/other permissions;
regular files must have one link. On Windows, handle security descriptors must
have the current process user as owner and a non-null DACL. Grants are restricted
to that user, Local System and Administrators. Other ACE forms are conservatively
rejected. Existing permissions are verified, never silently widened or rewritten.
Windows native execution still needs validation; x64 type/lint checks pass.

`HardwareOwner::acquire()` uses the user's application-data `Scanner/run/hardware.lock`
independently of database/listener selection. On macOS this is the installed
Swift service's lock location. A test verifies that the Rust standard-library
exclusive lock conflicts with the same `flock` operation used by Swift. Holding
this guard is required before constructing a real backend and through cleanup.
The file is retained after exit so another process cannot split lock ownership by
recreating it. `HardwareOwner::at` is available for isolated tests.

```sh
cargo test -p scanner-platform
cargo clippy -p scanner-platform --all-targets --target x86_64-pc-windows-msvc -- -D warnings
```

References: [Rust file locking](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock),
[Windows handle security](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-getsecurityinfo),
[Windows ACL entries](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getace).
Workspace path dependencies are intentional during migration; this crate is not
currently published independently.
