<div align="center">
<h1>Nyth</h1>
</div>

---

## The problem

Home Manager symlinks your configs out of the read-only Nix store. That's fine until some app
decides to write back to its own config file - a theme picker saving your last choice, a cache
sharing a file with the settings. Instead of a clean permission error it gets `EROFS`, and
depending on the app, it either crashes, corrupts something mid-write, or quietly forgets what
you told it.

## The session

nyth doesn't patch the apps or Home Manager. It opens a mount namespace with an overlayfs write
layer on top of your real `$HOME`, and runs your command inside it. Every managed config looks
ordinary and writable for as long as that command runs. When it ends, the namespace is gone -
outside it, nothing ever changed. What's left behind is a diff of whatever got written.

## Status and commit

`nyth status` shows that diff. `nyth commit` writes the keepers back to wherever they actually
belong - the real source file in your **local** dotfiles repo - and refuses, loudly, anything it doesn't
have a real destination for.

## Generated configs

Not everything in `$HOME` traces back to a file. `programs.git`, `programs.fish`, and most
`programs.*` modules render their config straight from Nix options, with no source file behind
them at all. A silent write there would just get overwritten on the next `home-manager switch`,
with you none the wiser. For those, nyth shows the change line by line instead and points you
back at the Nix options - without guessing which one, since reverse-engineering rendered config
back into an option path is a per-module problem with no general answer.

## Using it

Nothing about nyth is meant to be typed by hand. The Home Manager module reads your `home.file`
the same way Home Manager itself already does, and generates a wrapper with everything baked in:

```nix
{
  inputs.nyth.url = "github:FixeQD/nyth";

  # in your home-manager modules:
  imports = [ nyth.homeManagerModules.default ];
  programs.nyth = {
    enable = true;
    dotfilesRepo = "/home/you/your-flake-checkout";
  };
}
```

After that, `nyth-shell` is just a command:

```
nyth-shell session -- hyprctl reload
nyth-shell status
nyth-shell commit
```

`session` runs whatever you pass it inside the write-through namespace. `status` and `commit`
work on whatever's left behind afterward - you can run either one anytime, session active or
not, since the record of what changed lives outside the namespace on purpose.
