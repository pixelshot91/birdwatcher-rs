
# Doc of NixOs test
# https://nixos.org/manual/nixos/stable/index.html#ssec-machine-objects

conf_out = "/srv/birdwatcher-rs/birdwatcher_generated.conf"

machine.wait_for_unit("birdwatcher-rs.service", None, 5)
machine.execute("sleep 5")

print("Check 1: service shoud be false")
machine.copy_from_vm(conf_out, "1")
ret, stdout = machine.execute(f"cat {conf_out}")
assert ret == 0
assert stdout == '''
function my_service_fn() -> bool
{
    return false;
}
'''

print("Adding the the service script")
machine.execute("echo -e '#!/bin/sh\ntrue' > /tmp/service.sh")
machine.execute("chmod +x /tmp/service.sh")
machine.execute("sleep 1")

print("Check 2: service should still be false because we need 3 seconds to rise")
machine.copy_from_vm(conf_out, "2")
ret, stdout = machine.execute(f"cat {conf_out}")
assert ret == 0
assert stdout == '''
function my_service_fn() -> bool
{
    return false;
}
'''

machine.execute("sleep 5")

print("Check 3: service shoud be true by now")
machine.copy_from_vm(conf_out, "3")
ret, stdout = machine.execute(f"cat {conf_out}")
assert ret == 0
assert stdout == '''
function my_service_fn() -> bool
{
    return true;
}
'''