# Picking the right cross-compilation options

Finding the right [cross-compilation options](https://doc.rust-lang.org/rustc/targets/custom.html) for the kernel was a pain.
I made it a lot harder for myself by trying to generate a flat binary for my kernel.
`kernel/src/main.rs`, `kernel/x86_64-unknown-kernel.json`, and `kernel/kernel.ld` are the end results of a lot of fiddling.

## References

* [Redox's `x86_64-unknown-kernel` target](https://gitlab.redox-os.org/redox-os/kernel/-/blob/master/targets/x86_64-unknown-kernel.json?ref_type=heads)

  I used this as a base and removed settings that didn't affect compilation. I found `x86_64-unknown-none`
  unsuitable for generating a flat binary even when I overrode the linker to stop producing an ELF binary.

* [`TargetOptions`](https://doc.rust-lang.org/stable/nightly-rustc/rustc_target/spec/struct.TargetOptions.html>)
  struct in [`rustc_target`](https://doc.rust-lang.org/stable/nightly-rustc/rustc_target/).

  This is the reference for the fields that are allowed in the target JSON file.

* [LeonTheDev](https://old.reddit.com/user/LeonTheDev)'s answer in
  [this Reddit thread](https://old.reddit.com/r/osdev/comments/um3i0e/problems_executing_applications/i7zjt4u/)
  on linking flat binaries.

  I struggled with my kernel entrypoint shifting around; sometimes it would end up at the start of the binary,
  other times it wouldn't. LeonTheDev shared a (simple, in hindsight) solution
  ([linker script](http://web.archive.org/web/20220510092031/https://github.com/leon-robi/Trout/blob/main/Kernel/Linker.lds),
  [assembler code](http://web.archive.org/web/20220510092017/https://github.com/leon-robi/Trout/blob/main/Kernel/Kernel.asm))
  to get a flat binary that behaves properly regardless of the kernel entrypoint's location.
