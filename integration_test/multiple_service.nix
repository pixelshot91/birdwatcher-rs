{
  self,
  pkgs,
}:

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
          service_name = "service_true_name"
          function_name = "service_true_fn"
          command = ["/tmp/service_true.sh"]
          command_timeout_s = 1
          interval_s = 2
          fall = 1
          rise = 3

          [[service_definitions]]
          service_name = "service_false_name"
          function_name = "service_false_fn"
          command = ["/tmp/service_false.sh"]
          command_timeout_s = 1
          interval_s = 2
          fall = 2
          rise = 2

          [[service_definitions]]
          service_name = "service_timeout_name"
          function_name = "service_timeout_fn"
          command = ["/tmp/service_timeout.sh"]
          command_timeout_s = 1
          interval_s = 3
          fall = 2
          rise = 2
        '';
      };

      system.stateVersion = "23.11";
    };

  testScript = ''
    # machine.execute("journalctl -u birdwatcher-rs.service")
    machine.wait_for_unit("birdwatcher-rs.service", None, 5)
    print(machine.execute("journalctl -u birdwatcher-rs.service"))

    machine.copy_from_vm("/srv/birdwatcher-rs/birdwatcher_generated.conf", "1")
    machine.execute("sleep 5")
    machine.copy_from_vm("/srv/birdwatcher-rs/birdwatcher_generated.conf", "2")
    machine.execute("sleep 5")

    print("Adding the script files")
    machine.execute("echo -e '#!/bin/sh\ntrue' > /tmp/service_true.sh")
    machine.execute("echo -e '#!/bin/sh\nfalse' > /tmp/service_false.sh")
    machine.execute("echo -e '#!/bin/sh\nsleep 5' > /tmp/service_timeout.sh")
    machine.execute("chmod +x /tmp/service_*.sh")

    machine.execute("sleep 10")

    machine.copy_from_vm("/srv/birdwatcher-rs/birdwatcher_generated.conf", "3")
  '';
}
