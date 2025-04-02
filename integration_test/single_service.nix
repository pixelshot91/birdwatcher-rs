{
  self,
  pkgs,
}:
let
  conf_out = "/srv/birdwatcher-rs/birdwatcher_generated.conf";
in
pkgs.nixosTest {
  name = "birdwatcher-rs integration test";
  nodes.machine =
    { config, pkgs, ... }:
    {
      imports = [
        self.nixosModules.birdwatcher-rs-NixosModule
      ];
      services.birdwatcher-rs = {
        enable = true;
        config = ''
          generated_file_path = "${conf_out}"

          [bird_reload]
          command = ["birdc", "configure"]
          timeout_s = 2

          [[service_definitions]]
          service_name = "my_service_name"
          function_name = "my_service_fn"
          command = ["/tmp/service.sh"]
          command_timeout_s = 1
          interval_s = 1
          fall = 1
          rise = 3
        '';
      };

      system.stateVersion = "23.11";
    };

  testScript = builtins.readFile ./single_service.py;
}
