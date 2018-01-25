# Don't run this script! It's meant to run inside a VM, for testing purposes only.
# It messes with system files!

set -eu

cd /lib/x86_64-linux-gnu
rm -f libnss_loopback.so
cp /nsswitch_service/target/debug/examples/libnss_loopback.so libnss_loopback.so
chmod -x libnss_loopback.so
chown root:root libnss_loopback.so
rm -f libnss_loopback.so.2
ln -s libnss_loopback.so libnss_loopback.so.2
perl -pi -e 's/ dns$/ loopback dns/;' /etc/nsswitch.conf 
