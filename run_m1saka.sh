
cargo build
cargo bootloader-release

export M1N1DEVICE=/dev/cu.debug-console

sudo macvdmtool reboot serial
sleep 7
python3 /Users/mary/m1n1/proxyclient/raw_chainload.py m1_playground-release.bin
