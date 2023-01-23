# Mini Text
This is an attempt to create a text editor.
I declare this 'minimalist', but what I really mean is lacking
in features.

![image](https://user-images.githubusercontent.com/45665232/192302487-782e4667-ac25-4b95-9509-9c9ec062e411.png)


## Features
* Edit text
* Save files
* A Cursor
* Cursor Navigation

# How do I run it?
You've gotta have Rust installed. Go get it at [this link](https://www.rust-lang.org/).

Then, run with cargo :
```
cargo run
```


# Uh, but I'm on Ubuntu with Intel graphics.
Then DRI3 is probably not enabled. This may be required to run this.
So, go to `/etc/X11/xorg.conf.d/20-intel.conf`
and and `Option      "DRI"    "3"`
so it looks something like:
```
Section "Device"
  Identifier "Intel Graphics"
  Driver "intel"
  Option "DRI" "3"
EndSection
```
