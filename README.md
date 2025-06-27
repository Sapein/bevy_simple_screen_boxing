# bevy_simple_screen_boxing
[![Crates.io](https://img.shields.io/crates/v/bevy_simple_screen_boxing)](https://crates.io/crates/bevy_simple_screen_boxing)
[![docs.rs](https://docs.rs/bevy_simple_screen_boxing/badge.svg)](https://docs.rs/bevy_simple_screen_boxing/)
![License](https://img.shields.io/crates/l/bevy_simple_screen_boxing)

`bevy_simple_screen_boxing` aims to provide a relatively simple way to configure letterboxing and pillarboxing within Bevy.
It provides a simple component `CameraBox` which can be used to configure the behavior as you want.

## Features
- Provides a decent API for letterboxing/pillarboxing.

## Examples
### Integer Scaling
```rust
use bevy_simple_screen_boxing::CameraBoxingPlugin;
use bevy::prelude::*;

// Note, you will need to spawn the image.
fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Integer Scaling".into(),
                        name: Some("Integer Scaling".into()),
                        resolution: WindowResolution::new(1280., 720.),
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(CameraBoxingPlugin)
        .add_systems(Startup, setup);
}

fn setup(mut commands: Command) {
    let projection = OrthographicProjection::default_2d();
    projection.scaling_mode = ScalingMode::Fixed {
        width: 640.,
        height: 360.,
    };
    commands.spawn((
        Camera2d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::linear_rgb(0.5, 0.5, 0.9)),
            ..default()
        },
        CameraBox::ResolutionIntegerScale {
            resolution: Vec2::new(640., 360.),
            allow_imperfect_aspect_ratios: false,
        },
        Projection::Orthographic(projection)
    ));
}
```

## Known Limitations
- If you use multiple cameras, only one clear color can be displayed at once, even if they have different viewports.

## Supported Bevy Versions
| Bevy Simple Screen Boxing Version | Bevy Version |
|:---------------------------------:|:------------:|
|                0.1                |     0.16     |

## Acknowledgements  
I just want to thank the people who gave feedback on the initial Issue/PR even if it didn't make it in. That feedback
did shape and affect the overall design of this crate.

## QnA
### Why does this package exist?  
This actually comes from Bevy Issue #14158, which attempted to add in a set of commonly used resolutions for developers
to use. This was, unfortunately, rejected. However, it was decided that adding an easy way to do letter/pillar boxing
would be better, which can be found in #15130. This is my attempt at creating a potential API for the functionality.
