{
  config,
  lib,
  pkgs,
  # birdwatcher-rs,
  mypkgs,
  # a,
  ...
}:

with lib;
let
  cfg = config.services.birdwatcher-rs;
in
{
  options = {
    services.birdwatcher-rs = {
      enable = mkEnableOption "birdwatcher-rs";
    };
  };

  #### Implementation

  config = mkIf cfg.enable {
    users.users.birdwatcher-rs = {
      createHome = true;
      description = "birdwatcher-rs user";
      isSystemUser = true;
      group = "birdwatcher-rs";
      home = "/srv/birdwatcher-rs";
    };

    users.groups.birdwatcher-rs.gid = 1000;

    systemd.services.birdwatcher-rs = {
      description = "birdwatcher-rs server";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      script = ''
        exec ${pkgs.birdwatcher-rs}/bin/birdwatcher-rs \
      '';

      serviceConfig = {
        Type = "simple";
        User = "birdwatcher-rs";
        Group = "birdwatcher-rs";
        Restart = "on-failure";
        RestartSec = "30s";
      };
    };
  };
}
