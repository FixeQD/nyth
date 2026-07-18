# Home Manager module for nyth. Takes `self` so `programs.nyth.package` can default to this flake's own package
{ self }:
{ config, lib, pkgs, ... }:
let
  cfg = config.programs.nyth;

  # Home Manager already has to know this list to generate the $HOME symlinks in the first place
  watchedPaths = builtins.attrNames config.home.file;

  watchedPathArgs = lib.concatMapStringsSep " "
    (path: "--watched-path ${lib.escapeShellArg path}")
    watchedPaths;

  envArgs = lib.concatStringsSep " "
    (lib.mapAttrsToList
      (name: value: "--env ${lib.escapeShellArg "${name}=${value}"}")
      config.home.sessionVariables);

  # The full watched-path/env list is baked into this script as literal argv at build time
  nythShell = pkgs.writeShellApplication {
    name = "nyth-shell";
    runtimeInputs = [ cfg.package ];
    text = ''
      exec nyth session ${watchedPathArgs} ${envArgs} -- "$@"
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
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ nythShell ];
  };
}
