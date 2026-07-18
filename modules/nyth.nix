# Home Manager module for nyth. Takes `self` so `programs.nyth.package` can default to this flake's own package
{ self }:
{ config, lib, pkgs, ... }:
let
  cfg = config.programs.nyth;

  # Home Manager already has to know this list to generate the $HOME symlinks in the first place
  allFiles = config.home.file;
  watchedPaths = builtins.attrNames allFiles;

  isGenerated = _name: fileCfg: fileCfg.text != null;

  repoBackedPaths = builtins.attrNames (lib.filterAttrs (n: v: !isGenerated n v) allFiles);
  generatedPaths = builtins.attrNames (lib.filterAttrs isGenerated allFiles);

  watchedPathArgs = lib.concatMapStringsSep " "
    (path: "--watched-path ${lib.escapeShellArg path}")
    watchedPaths;

  repoBackedArgs = lib.concatMapStringsSep " "
    (path: "--repo-backed ${lib.escapeShellArg path}")
    repoBackedPaths;

  generatedArgs = lib.concatMapStringsSep " "
    (path: "--generated ${lib.escapeShellArg path}")
    generatedPaths;

  envArgs = lib.concatStringsSep " "
    (lib.mapAttrsToList
      (name: value: "--env ${lib.escapeShellArg "${name}=${value}"}")
      config.home.sessionVariables);

  nythShell = pkgs.writeShellApplication {
    name = "nyth-shell";
    runtimeInputs = [ cfg.package ];
    text = ''
      cmd="''${1:-}"
      case "$cmd" in
        session)
          shift
          exec nyth session ${watchedPathArgs} ${envArgs} -- "$@"
          ;;
        status|commit)
          shift
          exec nyth "$cmd" \
            --repo-root ${lib.escapeShellArg cfg.dotfilesRepo} \
            ${repoBackedArgs} ${generatedArgs} \
            "$@"
          ;;
        *)
          echo "usage: nyth-shell <session -- <command> | status | commit>" >&2
          exit 1
          ;;
      esac
    '';
  };
in
{
  options.programs.nyth = {
    enable = lib.mkEnableOption "write-through OverlayFS session over $HOME managed by Home Manager";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "nyth.packages.<system>.default";
      description = "The `nyth` package providing the `nyth` binary";
    };

    dotfilesRepo = lib.mkOption {
      type = lib.types.str;
      description = ''
        Absolute path to the local, on-disk checkout of your flake's dotfiles repo where `nyth commit` writes repo-backed changes back to.
        Nyth hasno way to derive this from the flake evaluation itself, since that only ever sees paths already copied into the store
      '';
      example = "/home/pawel/nixos-config";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ nythShell ];
  };
}
