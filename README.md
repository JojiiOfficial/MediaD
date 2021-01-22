# MediaD
A very simple and lightweight deamon to handle keyboard media button events easily.

# Permissions
### Testing
Run followig command. Replace 'USER' with your username and 'DEVICE' with the device you want to let your music get controlled from
```bash
sudo setfacl -m u:<USER>:rw /dev/input/by-id/<DEVICE>
```

### Permanent changes
Paste the following snippet into `/etc/udev/rules.d/99-userdev-input.rules`:
```
KERNEL=="event*", SUBSYSTEM=="input", RUN+="/usr/bin/setfacl -m u:<USER>:rw /dev/input/by-id/<DEVICE>"
```
Reboot, or run the command from [Testing](#Testing) to apply changes

# Installation

## Pacman repo
You can get it precompiled from my [pacman repository](https://repo.jojii.de)

## AUR
`yay -S mediad`

## Compilation
#### Requirements (make depends)
Arch: libpulse dbus <br>
Fedora: dbus-devel 
<br>

Run following command:
```
cargo install mediad
```

# Usage
```
mediad <DEVICE>
```
Where 'DEVICE' is the same device you used earlier to give the user permissions
