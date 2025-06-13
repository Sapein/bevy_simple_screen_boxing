# bevy_simple_screen_boxing
[![Crates.io](https://img.shields.io/crates/v/bevy_simple_screen_boxing)](https://crates.io/crates/bevy_simple_screen_boxing)
[![docs.rs](https://docs.rs/bevy_simple_screen_boxing/badge.svg)](https://docs.rs/bevy_simple_screen_boxing/)
![License](https://img.shields.io/crates/l/bevy_simple_screen_boxing)

`bevy_simple_screen_boxing` aims to provide a relatively simple way to configure letterboxing and pillarboxing within Bevy.
It provides a simple component `CameraBox` which can be used to configure the behavior as you want.

## Features
- Provides a decent API for letterboxing/pillarboxing.

## Known Limitations
- If you use multiple cameras, only one clear color can be displayed at once, even if they have different viewports.

## Supported Bevy Versions
| Bevy Resolution Version | Bevy Version |
|:-----------------------:|:------------:|
|          0.1.0          |     0.16     |

## Acknowledgements  
I just want to thank the people who gave feedback on the initial Issue/PR even if it didn't make it in. That feedback
did shape and affect the overall design of this crate.

## QnA
### Why does this package exist?  
This actually comes from Bevy Issue #14158, which attempted to add in a set of commonly used resolutions for developers
to use. This was, unfortunately, rejected. However, it was decided that adding an easy way to do letter/pillar boxing
would be better, which can be found in #15130. This is my attempt at creating a potential API for the functionality.