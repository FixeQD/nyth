<div align="center">
<h1>Nyth</h1>
</div>

## The problem

Home Manager symlinks your configs out of the read-only Nix store. That's fine until some app
decides to write back to its own config file - a theme picker saving your last choice, a cache
sharing a file with the settings. Instead of a clean permission error it gets `EROFS`, and
depending on the app, it either crashes, corrupts something mid-write, or quietly forgets what
you told it.

## The overlay

Nyth doesn't patch the apps or Home Manager. `nyth mount --for-user <name>`, run as root, sets
up a persistent OverlayFS write layer over that user's real `$HOME`: every managed config looks
ordinary and writable, for as long as the mount lives - not just for one login. Nothing about
nyth is a session tied to a login; the mount is a durable piece of system state, the same as any
other mount on the box, until `nyth unmount --for-user <name>` or a reboot takes it down. What's
left behind on top is a diff of whatever got written.

## Status and commit

`nyth status --for-user <name>` shows that diff. `nyth commit --for-user <name>` writes the
keepers back to wherever they actually belong - the real source file in your **local** dotfiles
repo - and refuses, loudly, anything it doesn't have a real destination for. Both read
`/run/nyth/<name>/upper` directly and don't care whether the overlay is currently mounted or not.

## Generated configs

Not everything in `$HOME` traces back to a file. `programs.git`, `programs.fish`, and most
`programs.*` modules render their config straight from Nix options, with no source file behind
them at all. A silent write there would just get overwritten on the next `home-manager switch`,
with you none the wiser. For those, nyth shows the change line by line instead and points you
back at the Nix options - without guessing which one, since reverse-engineering rendered config
back into an option path is a per-module problem with no general answer.

## Using it

Nyth is a plain binary: four independent commands, no config file, no daemon, no idea what init system (if any) called it

```
nyth mount   --for-user <name> [--watched-path <rel> ...]
nyth unmount --for-user <name> [--purge]
nyth status  --for-user <name> --repo-root <path> [--repo-backed <rel> ...] [--generated <rel> ...]
nyth commit  --for-user <name> --repo-root <path> [--repo-backed <rel> ...] [--generated <rel> ...]
```

`mount`/`unmount` need root - they act on someone else's `$HOME`. `status`/`commit` only
read `/run/nyth/<name>/upper`, which `mount` leaves readable/writable by its owner, so those run
fine as a normal user, no `sudo`.

The Home Manager module reads your `home.file` the same way Home Manager itself already does,
and wires up two commands you run by hand:

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

```
nyth-status
nyth-commit
```

They work on whatever's in the overlay right now - you can run either one anytime, mount active
or not, since the record of what changed lives in `/run/nyth/<name>/upper` on purpose,
independent of the overlay's own lifecycle.

## Mounting at boot

Something has to call `nyth mount --for-user <name>` when the system starts (or at first login,
if you'd rather do it lazily :P). That something is unavoidably tied to your init system, and nyth
deliberately doesn't provide it - a project that generates systemd/finit units to trigger its own
binary would itself depend on a particular init system, exactly what nyth avoids everywhere else.
Instead, the Home Manager module writes the flags `nyth mount` needs to
`~/.local/state/nyth/mount-args`, and you write your own unit that reads that file. On systemd:

```nix
# in your own NixOS config, NOT in the nyth flake - this is your integration, not nyth's
systemd.services."nyth-mount-user" = {
  description = "nyth write-through overlay for user";
  wantedBy = [ "multi-user.target" ];
  after = [ "local-fs.target" ];
  serviceConfig = {
    Type = "oneshot";
    RemainAfterExit = true;
    ExecStart = "/bin/sh -c 'exec ${pkgs.nyth}/bin/nyth mount --for-user user $(cat /home/user/.local/state/nyth/mount-args)'";
    ExecStop = "${pkgs.nyth}/bin/nyth unmount --for-user user";
  };
};
```

Swap in the equivalent finit task, a manual `sudo nyth mount ...`, a cron job - nyth genuinely
doesn't care. It never checks who or what invoked it; it takes argv, makes syscalls and exits
