{ self, pkgs }:

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
          generated_file_path = "/srv/birdwatcher-rs/birdwatcher_generated.conf"

          [bird_reload]
          command = ["birdc", "configure"]
          timeout_s = 2

          [[service_definitions]]
          service_name = "first_service"
          function_name = "match_true"
          # command = []
          command = ["/bin/ls", "1"]
          command_timeout_s = 1
          interval_s = 1.2
          fall = 1
          rise = 3

          [[service_definitions]]
          service_name = "second_service"
          function_name = "match_false"
          command = ["/bin/sleep", "2"]
          command_timeout_s = 1
          interval_s = 2
          fall = 2
          rise = 2
        '';
      };

      system.stateVersion = "23.11";
    };

  testScript = ''
    machine.wait_for_unit("birdwatcher-rs.service")
    machine.screenshot("my_screen_1.png")
    machine.wait_for_open_port(3000, 'localhost', 2)
  '';
}
