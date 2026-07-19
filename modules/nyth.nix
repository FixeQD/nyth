# Home Manager module for nyth. Takes `self` so `programs.nyth.package` can default to this flake's own package
{ self }:
{ config, lib, pkgs, ... }:
let
  cfg = config.programs.nyth;

  # Home Manager already has to know this list to generate the $HOME symlinks in the first place
  allFiles = config.home.file;

  walkRecursiveSource = source: prefix:
    lib.concatMapAttrs
      (name: type:
        let
          path = source + "/${name}";
          rel = if prefix == "" then name else "${prefix}/${name}";
        in
        if type == "directory" then walkRecursiveSource path rel else { ${rel} = null; }
      )
      (builtins.readDir source);

  normalizeName = name:
    if lib.hasPrefix "/" name then
      let homePrefix = "${config.home.homeDirectory}/"; in
      if lib.hasPrefix homePrefix name then lib.removePrefix homePrefix name else null
    else
      name;

  expandFile = name: fileCfg:
    if (fileCfg.recursive or false) && fileCfg.source != null then
      map (rel: "${name}/${rel}") (builtins.attrNames (walkRecursiveSource fileCfg.source ""))
    else
      [ name ];

  expanded = builtins.filter (e: e != null) (
    lib.mapAttrsToList
      (name: fileCfg:
        let relName = normalizeName name; in
        if relName == null then null
        else {
          paths = expandFile relName fileCfg;
          generated = fileCfg.text != null;
        }
      )
      allFiles
  );

  watchedPaths = lib.concatMap (e: e.paths) expanded;
  repoBackedPaths = lib.concatMap (e: if e.generated then [ ] else e.paths) expanded;
  generatedPaths = lib.concatMap (e: if e.generated then e.paths else [ ]) expanded;

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
      (name: value: "--env ${lib.escapeShellArg "${name}=${toString value}"}")
      config.home.sessionVariables);

  nythShell = pkgs.writeShellApplication {
    name = "nyth-shell";
    runtimeInputs = [ cfg.package ];
    text = ''
      cmd="''${1:-}"
      case "$cmd" in
        session)
          shift
          exec nyth session ${watchedPathArgs} ${envArgs} "$@"
          ;;
        status|commit)
          shift
          exec nyth "$cmd" \
            --repo-root ${lib.escapeShellArg cfg.dotfilesRepo} \
            ${repoBackedArgs} ${generatedArgs} \
            "$@"
          ;;
        *)
          echo "usage: nyth-shell <session [-- <command>] | status | commit>" >&2
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
