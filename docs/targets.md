# Picking the right cross-compilation options

Finding the right [cross-compilation options](https://doc.rust-lang.org/rustc/targets/custom.html) for the kernel was a pain.
I made it harder for myself by trying to generate a flat binary for my kernel.
`kernel/x86_64-unknown-kernel.json` is the end result of a lot of fiddling.

In particular, `"relocation-model": "static"` and `"static-position-independent-executables": "false"` seem necessary,
and `"static-position-independent-executables": "false"` seems to be implicated in a vicious bug.
For example, changing `"false"` to `false` gives me the wrong binary, but `"true"` also gives me the wrong binary.
Moving `"static-position-independent-executables": "false"` elsewhere in the file gives me the wrong binary.
And removing the field also gives me the wrong binary.

The `gnu-lld` link flavor also seems necessary for achieving the right output (using `ld.lld` breaks), but given
how brittle the `static-position-independent-executables` is, this may be a red herring.

## Resources

* [Redox OS target definitions](https://gitlab.redox-os.org/redox-os/kernel/-/blob/master/targets?ref_type=heads)

  Redox is an OS written in Rust, so I figured they must have some something similar.

* [`TargetOptions`](https://doc.rust-lang.org/stable/nightly-rustc/rustc_target/spec/struct.TargetOptions.html>)
  struct in [`rustc_target`](https://doc.rust-lang.org/stable/nightly-rustc/rustc_target/).

  This is the reference for the fields that are allowed in the target JSON file.
