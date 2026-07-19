{ config, lib, ... }:
let
  cfg = config.programs.nyth;
in
{
  options.programs.nyth = {
    enable = lib.mkEnableOption ''
      the /etc/profile.d hook that starts a nyth overlay session at login.
    '';
  };

  config = lib.mkIf cfg.enable {
    # nyth session does unshare(CLONE_NEWUSER | CLONE_NEWNS) unprivileged
    security.allowUserNamespaces = true;

    environment.etc."profile.d/nyth.sh".text = ''
      if [ -z "''${NYTH_SESSION_ACTIVE-}" ] && command -v nyth-shell >/dev/null 2>&1; then
          export NYTH_SESSION_ACTIVE=1
          exec nyth-shell session
      fi
    '';
  };
}
