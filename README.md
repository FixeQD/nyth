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

Nyth doesn't patch the apps or Home Manager. It opens a mount namespace with an overlayfs write
layer on top of your real `$HOME`, and runs inside it - your login shell, so everything you do
for the rest of that login lives inside the same overlay. Every managed config looks ordinary
and writable the whole time. When the namespace ends, so does the overlay - outside it, nothing
ever changed. What's left behind is a diff of whatever got written.

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

The NixOS module is one line:

```nix
{
  inputs.nyth.url = "github:FixeQD/nyth";
  imports = [ nyth.nixosModules.default ];
  programs.nyth.enable = true;
}
```

With both in place, you never run `session` yourself - the NixOS module opens it at login
through `/etc/profile.d/nyth.sh`, before your display manager or WM autostart, so they inherit
the overlay instead of starting fresh. `status` and `commit` are the only things you run by
hand:

```
nyth-shell status
nyth-shell commit
```

They work on whatever's left behind in the overlay - you can run either one anytime, session
active or not, since the record of what changed lives outside the namespace on purpose.
