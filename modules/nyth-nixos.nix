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
    boot.kernel.sysctl."kernel.unprivileged_userns_clone" = lib.mkDefault 1;

    environment.etc."profile.d/nyth.sh".text = ''
      # nyth-shell only exists for users with programs.nyth.enable in their own
      # Home Manager config — everyone else's login runs through untouched.
      if [ -z "''${NYTH_SESSION_ACTIVE-}" ] && command -v nyth-shell >/dev/null 2>&1; then
          export NYTH_SESSION_ACTIVE=1
          exec nyth-shell session
      fi
    '';
  };
}
