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

  repoBackedPaths = lib.concatMap (e: if e.generated then [ ] else e.paths) expanded;
  generatedPaths = lib.concatMap (e: if e.generated then e.paths else [ ]) expanded;

  repoBackedArgs = lib.concatMapStringsSep " "
    (path: "--repo-backed ${lib.escapeShellArg path}")
    repoBackedPaths;
  generatedArgs = lib.concatMapStringsSep " "
    (path: "--generated ${lib.escapeShellArg path}")
    generatedPaths;

  nythStatusCmd = pkgs.writeShellApplication {
    name = "nyth-status";
    runtimeInputs = [ cfg.package ];
    text = ''
      exec nyth status --for-user ${lib.escapeShellArg config.home.username} \
        --repo-root ${lib.escapeShellArg cfg.dotfilesRepo} \
        ${repoBackedArgs} ${generatedArgs} "$@"
    '';
  };
  nythCommitCmd = pkgs.writeShellApplication {
    name = "nyth-commit";
    runtimeInputs = [ cfg.package ];
    text = ''
      exec nyth commit --for-user ${lib.escapeShellArg config.home.username} \
        --repo-root ${lib.escapeShellArg cfg.dotfilesRepo} \
        ${repoBackedArgs} ${generatedArgs} "$@"
    '';
  };
in
{
  options.programs.nyth = {
    enable = lib.mkEnableOption "write-through OverlayFS nad $HOME zarządzanym przez Home Managera";

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
        Nyth has no way to derive this from the flake evaluation itself, since that only ever sees paths already copied into the store.
      '';
      example = "/home/user/nixos-config";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ nythStatusCmd nythCommitCmd ];
    home.file.".local/state/nyth/mount-args".text =
      "--home-files ${lib.escapeShellArg config.home-files}";
  };
}
