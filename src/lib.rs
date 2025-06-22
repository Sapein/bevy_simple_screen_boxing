//! `bevy_simple_screen_boxing` aims to provide a simple, easy, and convenient way to set and manage
//! camera boxing (that is, letterboxing and pillarboxing) in order to ensure that the output is
//! within the right resolution or aspect ratio.
//!
//! It provides ways to set a singular static resolution or aspect ratio, to always ensure the output
//! is at a resolution that is an integer scale, or provide manually specified letter/pillarboxing.
//!
//! This crate requires bevy version `0.16`
//!
//! ## Features
//! - Provides an easy, but powerful, API for camera boxing!
//!
//! ## Quick Start
//! - Add the `CameraBoxingPlugin`
//! - Add the `CameraBox` component to your Camera, and configure what you need.

use bevy_app::{App, First, Plugin};
use bevy_asset::{AssetEvent, Assets};
use bevy_ecs::prelude::*;
use bevy_image::Image;
use bevy_log::{info, warn};
use bevy_math::{AspectRatio, UVec2, Vec2};
use bevy_reflect::Reflect;
use bevy_render::camera::{ManualTextureViews, Viewport};
use bevy_render::prelude::*;
use bevy_window::{PrimaryWindow, Window};

/// The Plugin that adds in all the systems for camera-boxing.
pub struct CameraBoxingPlugin;
impl Plugin for CameraBoxingPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CameraBox>()
            .add_event::<AdjustBoxing>()
            .add_systems(First, (windows_changed, camerabox_changed))
            .add_systems(First, images_changed.run_if(on_event::<AssetEvent<Image>>))
            .add_systems(
                First,
                texture_views_changed.run_if(resource_changed_or_removed::<ManualTextureViews>),
            )
            .add_systems(
                First,
                adjust_viewport
                    .run_if(on_event::<AdjustBoxing>)
                    .after(camerabox_changed)
                    .after(windows_changed)
                    .after(images_changed)
                    .after(texture_views_changed),
            );
    }
}

#[derive(Component, Reflect)]
#[reflect(Component)]
/// Configures how to box the output, with either: PillarBoxes, Letterboxes, or both.
pub enum CameraBox {
    /// Keep the output at a static resolution, if possible, and box if it exceeds the resolution.
    /// If the output is smaller than the resolution, it will output at the smaller resolution
    /// instead.
    StaticResolution {
        resolution: UVec2,

        /// Where to put the Boxed output, if this is None then it will be centered.
        position: Option<UVec2>,
    },

    /// Keep the output as a static Aspect Ratio. If the output is not at the Aspect Ratio apply
    /// boxing to force it into the correct Aspect Ratio.
    StaticAspectRatio {
        aspect_ratio: AspectRatio,

        /// Where to put the Boxed output, if this is None then it will be centered.
        position: Option<UVec2>,
    },

    /// Keep the output at an Integer Scale of a specific Resolution, if no Integer Scale exists
    /// box the output to an Integer Scale.
    ResolutionIntegerScale {
        resolution: Vec2,

        /// If this is true, then the output may not be *exactly* the proper Aspect Ratio if the
        /// output resolution is smaller than the resolution specified, this will result in only
        /// letterboxing or pillarboxing, but not windowboxing.
        ///
        /// If this is false, then we will use a second method which will ensure that the Aspect
        /// Ratio for the smaller output *will* be as exact as we can get it, and will ensure that
        /// the output would be windowboxed properly.
        ///
        /// If the output resolution is expected to larger than, or equal to, the resolution
        /// specified then this does not matter.
        allow_imperfect_aspect_ratios: bool,
    },

    /// Have static letterboxing with specific sizes for each of the bars.
    LetterBox {
        /// The bar at the top of the output.
        top: u32,

        /// The bar at the bottom of the output.
        bottom: u32,

        /// Whether we can attempt to scale the letterboxing if the output is smaller than
        /// the desired sizing. If we can't, it will disable letterboxing when it can not accommodate
        /// the sizes requested.
        strict_letterboxing: bool,
    },

    /// Have static Pillarboxing with specific sizes for each of the bars.
    PillarBox {
        /// The bar on the left side of the output.
        left: u32,

        /// The bar on the right side of the output.
        right: u32,
    },
}

#[derive(Event)]
struct AdjustBoxing;

fn windows_changed(
    mut boxing_event: EventWriter<AdjustBoxing>,
    window: Query<&Window, Changed<Window>>,
) {
    if !window.is_empty() {
        boxing_event.write(AdjustBoxing);
    }
}

fn images_changed(mut boxing_event: EventWriter<AdjustBoxing>) {
    boxing_event.write(AdjustBoxing);
}

fn texture_views_changed(mut boxing_event: EventWriter<AdjustBoxing>) {
    boxing_event.write(AdjustBoxing);
}

type CameraChanged = Or<(Changed<CameraBox>, Changed<Camera>)>;
fn camerabox_changed(
    mut boxing_event: EventWriter<AdjustBoxing>,
    boxes: Query<&CameraBox, CameraChanged>,
) {
    if !boxes.is_empty() {
        boxing_event.write(AdjustBoxing);
    }
}

fn adjust_viewport(
    mut boxed_cameras: Query<(&mut Camera, &CameraBox)>,
    primary_window: Option<Single<Entity, With<PrimaryWindow>>>,
    windows: Query<(Entity, &Window)>,
    texture_views: Res<ManualTextureViews>,
    images: Res<Assets<Image>>,
) {
    let primary_window = primary_window.map(|e| e.into_inner());
    for (mut camera, camera_box) in boxed_cameras.iter_mut() {
        let target = camera.target.normalize(primary_window);

        let target = match target
            .and_then(|t| t.get_render_target_info(windows, &images, &texture_views))
        {
            None => {
                info!(
                    "Failed to get normalized render target! Are you rendering to a Primary Window without having set one?"
                );
                continue;
            }
            Some(target) => target,
        };

        match &camera_box {
            CameraBox::StaticResolution {
                resolution: size,
                position,
            } => {
                let mut viewport = match &mut camera.viewport {
                    None => Viewport::default(),
                    Some(viewport) => viewport.to_owned(),
                };

                if &viewport.physical_size != size {
                    viewport.physical_size = size.clamp(UVec2::ONE, target.physical_size);
                }

                viewport.physical_position = if position
                    .is_none_or(|u| u != viewport.physical_position)
                {
                    (target.physical_size - viewport.physical_size / 2).min(target.physical_size)
                } else {
                    position.unwrap()
                };
                camera.viewport = Some(viewport);
            }
            CameraBox::StaticAspectRatio {
                aspect_ratio,
                position,
            } => {
                let mut viewport = match &mut camera.viewport {
                    None => Viewport::default(),
                    Some(viewport) => viewport.to_owned(),
                };

                let physical_aspect_ratio =
                    match AspectRatio::try_from(target.physical_size.as_vec2()) {
                        Ok(ar) if ar.ratio() == aspect_ratio.ratio() => {
                            camera.viewport = None;
                            continue;
                        }
                        Err(e) => {
                            warn!(
                                "Error occurred when calculating aspect ratios for scaling: {:?}",
                                e
                            );
                            continue;
                        }
                        Ok(ar) => ar,
                    };

                let Boxing {
                    boxing_offset,
                    output_resolution: render_size,
                } = calculate_boxing_from_aspect_ratios(
                    &target.physical_size.as_vec2(),
                    &physical_aspect_ratio,
                    aspect_ratio,
                );

                viewport.physical_position = match position {
                    None => boxing_offset.as_uvec2(),
                    Some(pos) => *pos,
                };
                viewport.physical_size = render_size.as_uvec2();
                camera.viewport = Some(viewport);
            }

            CameraBox::ResolutionIntegerScale {
                allow_imperfect_aspect_ratios,
                resolution,
            } => {
                let mut viewport = match &mut camera.viewport {
                    None => Viewport::default(),
                    Some(viewport) => viewport.to_owned(),
                };
                let Boxing {
                    boxing_offset,
                    output_resolution,
                } = match if *allow_imperfect_aspect_ratios {
                    calculate_boxing_imperfect(&target.physical_size.as_vec2(), resolution)
                } else {
                    calculate_boxing_perfect(&target.physical_size.as_vec2(), resolution)
                } {
                    Ok(None) => {
                        camera.viewport = None;
                        continue;
                    }
                    Ok(Some(t)) => t,
                    Err(e) => {
                        warn!(
                            "Error occurred when calculating aspect ratios for scaling: {:?}",
                            e
                        );
                        continue;
                    }
                };

                viewport.physical_position = boxing_offset.as_uvec2();
                viewport.physical_size = output_resolution.as_uvec2();
                camera.viewport = Some(viewport);
            }
            CameraBox::LetterBox {
                top,
                bottom,
                strict_letterboxing,
            } => {
                let mut viewport = match &camera.viewport {
                    None => Viewport::default(),
                    Some(viewport) => viewport.to_owned(),
                };

                let Boxing {
                    mut boxing_offset,
                    mut output_resolution,
                } = calculate_letterbox(&target.physical_size.as_vec2(), (top, bottom));
                if (output_resolution.y + boxing_offset.y > target.physical_size.y as f32
                    || output_resolution.y <= 0.)
                    && !strict_letterboxing
                {
                    output_resolution.y = target.physical_size.y as f32 / 2.;
                    boxing_offset.y /= 2.;
                    let scale_factor =
                        (target.physical_size.y as f32) / (output_resolution.y + boxing_offset.y);
                    boxing_offset.y *= scale_factor;
                }

                if (output_resolution.y <= 0.
                    || output_resolution.y > target.physical_size.y as f32
                    || output_resolution.y + boxing_offset.y > target.physical_size.y as f32)
                    && *strict_letterboxing
                {
                    output_resolution.y = target.physical_size.y as f32;
                    output_resolution.x = target.physical_size.x as f32;
                    boxing_offset.y = 0.;
                }

                viewport.physical_position = boxing_offset.as_uvec2();
                viewport.physical_size = output_resolution.as_uvec2();
                camera.viewport = Some(viewport);
            }
            CameraBox::PillarBox { left, right } => {
                let mut viewport = match &mut camera.viewport {
                    None => Viewport::default(),
                    Some(viewport) => viewport.to_owned(),
                };

                let Boxing {
                    mut boxing_offset,
                    mut output_resolution,
                } = calculate_pillarbox(&target.physical_size.as_vec2(), (left, right));

                if output_resolution.x <= 0.
                    || output_resolution.x > target.physical_size.x as f32
                    || output_resolution.x + boxing_offset.x > target.physical_size.x as f32
                {
                    output_resolution.x = target.physical_size.x as f32;
                    output_resolution.x = target.physical_size.x as f32;
                    boxing_offset.x = 0.;
                }

                viewport.physical_position = boxing_offset.as_uvec2();
                viewport.physical_size = output_resolution.as_uvec2();
                camera.viewport = Some(viewport);
            }
        }
    }
}

#[derive(PartialEq, Debug)]
struct Boxing {
    boxing_offset: Vec2,
    output_resolution: Vec2,
}

fn calculate_boxing_from_aspect_ratios(
    physical_size: &Vec2,
    physical_aspect_ratio: &AspectRatio,
    target_aspect_ratio: &AspectRatio,
) -> Boxing {
    if physical_aspect_ratio.ratio() > target_aspect_ratio.ratio() {
        let render_height = physical_size.y;
        let render_width = render_height * target_aspect_ratio.ratio();
        Boxing {
            boxing_offset: Vec2::new(physical_size.x / 2. - render_width / 2., 0.),
            output_resolution: Vec2::new(render_width, render_height),
        }
    } else {
        let render_width = physical_size.x;
        let render_height = render_width / target_aspect_ratio.ratio();
        Boxing {
            boxing_offset: Vec2::new(0., physical_size.y / 2. - render_height / 2.),
            output_resolution: Vec2::new(render_width, render_height),
        }
    }
}
fn calculate_boxing_imperfect(physical_size: &Vec2, desired_size: &Vec2) -> Result<Option<Boxing>> {
    let desired_aspect_ratio = AspectRatio::try_from(*desired_size)?;
    let physical_aspect_ratio = AspectRatio::try_from(*physical_size)?;

    //NOTE: this does not really handle the case where the target size is smaller than the desired height/width.
    let height_scale = physical_size.y / desired_size.y;
    let width_scale = physical_size.x / desired_size.x;

    let small_height_scale = desired_size.y / physical_size.y;
    let small_width_scale = desired_size.x / physical_size.x;

    let has_int_scale = desired_aspect_ratio.ratio() == physical_aspect_ratio.ratio()
        && ((height_scale % 1. == 0. && width_scale % 1. == 0.)
            || (small_height_scale % 1. == 0. && small_width_scale % 1. == 0.));

    // Integer Scaling Exists
    if has_int_scale {
        return Ok(None);
    }

    let best_scale = if width_scale > height_scale {
        height_scale
    } else {
        width_scale
    };

    let render_width = if best_scale >= 1. {
        desired_size.x * best_scale.floor()
    } else {
        desired_size.x * best_scale
    };

    let render_height = if best_scale >= 1. {
        desired_size.y * best_scale.floor()
    } else {
        desired_size.y * best_scale
    };

    let letterbox_size = physical_size.y - render_height;
    let pillarbox_size = physical_size.x - render_width;

    Ok(Some(Boxing {
        boxing_offset: Vec2::new(pillarbox_size / 2., letterbox_size / 2.),
        output_resolution: Vec2::new(render_width, render_height),
    }))
}
fn calculate_boxing_perfect(physical_size: &Vec2, desired_size: &Vec2) -> Result<Option<Boxing>> {
    let desired_aspect_ratio = AspectRatio::try_from(*desired_size)?;
    let physical_aspect_ratio = AspectRatio::try_from(*physical_size)?;

    let height_scale = physical_size.y / desired_size.y;
    let width_scale = physical_size.x / desired_size.x;

    let has_int_scale = desired_aspect_ratio.ratio() == physical_aspect_ratio.ratio()
        && (height_scale % 1. == 0. && width_scale % 1. == 0.);

    // Integer Scaling Exists
    if has_int_scale {
        return Ok(None);
    }

    if height_scale < 1. || width_scale < 1. {
        let height_scale = desired_size.y / physical_size.y;
        let width_scale = desired_size.x / physical_size.x;

        // Recheck with the current values
        let has_int_scale = desired_aspect_ratio.ratio() == physical_aspect_ratio.ratio()
            && (height_scale % 1. == 0. && width_scale % 1. == 0.);

        // Integer Scaling Exists
        if has_int_scale {
            return Ok(None);
        }

        let best_divisor = if height_scale < width_scale {
            width_scale
        } else {
            height_scale
        }
        .ceil();

        let render_height = desired_size.y / best_divisor;
        let render_width = desired_size.x / best_divisor;

        let letterbox_size = physical_size.y - render_height;
        let pillarbox_size = physical_size.x - render_width;
        Ok(Some(Boxing {
            boxing_offset: Vec2::new(pillarbox_size / 2., letterbox_size / 2.),
            output_resolution: Vec2::new(render_width, render_height),
        }))
    } else {
        let best_scale = if width_scale > height_scale {
            height_scale
        } else {
            width_scale
        }
        .floor();

        let render_width = desired_size.x * best_scale;
        let render_height = desired_size.y * best_scale;

        let letterbox_size = physical_size.y - render_height;
        let pillarbox_size = physical_size.x - render_width;
        Ok(Some(Boxing {
            boxing_offset: Vec2::new(pillarbox_size / 2., letterbox_size / 2.),
            output_resolution: Vec2::new(render_width, render_height),
        }))
    }
}

fn calculate_letterbox(physical_size: &Vec2, letterbox: (&u32, &u32)) -> Boxing {
    let letterbox_height = (letterbox.0 + letterbox.1) as f32;
    let render_width = physical_size.x;
    let render_height = physical_size.y - letterbox_height;

    Boxing {
        boxing_offset: Vec2::new(0., *letterbox.0 as f32),
        output_resolution: Vec2::new(render_width, render_height),
    }
}

fn calculate_pillarbox(physical_size: &Vec2, pillarbox: (&u32, &u32)) -> Boxing {
    let pillarbox_width = (pillarbox.0 + pillarbox.1) as f32;
    let render_height = physical_size.y;
    let render_width = physical_size.x - pillarbox_width;

    Boxing {
        boxing_offset: Vec2::new(*pillarbox.0 as f32, 0.),
        output_resolution: Vec2::new(render_width, render_height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    impl Boxing {
        fn new(boxing_offset: Vec2, output_resolution: Vec2) -> Self {
            Boxing {
                boxing_offset,
                output_resolution,
            }
        }
    }

    mod internal {
        use super::*;

        #[test]
        fn test_aspect_ratio_scaling() -> Result<()> {
            assert_eq!(
                calculate_boxing_from_aspect_ratios(
                    &Vec2::new(640., 360.),
                    &AspectRatio::try_new(640., 360.)?,
                    &AspectRatio::try_new(640., 360.)?
                ),
                Boxing::new(Vec2::ZERO, Vec2::new(640., 360.))
            );
            
            assert_eq!(
                calculate_boxing_from_aspect_ratios(
                    &Vec2::new(1280., 720.),
                    &AspectRatio::try_new(1280., 720.)?,
                    &AspectRatio::try_new(640., 360.)?
                ),
                Boxing::new(Vec2::ZERO, Vec2::new(1280., 720.))
            );

            assert_eq!(
                calculate_boxing_from_aspect_ratios(
                    &Vec2::new(1920., 1080.),
                    &AspectRatio::try_new(1920., 1080.)?,
                    &AspectRatio::try_new(1280., 720.)?
                ),
                Boxing::new(Vec2::ZERO, Vec2::new(1920., 1080.))
            );
            
            assert_eq!(
                calculate_boxing_from_aspect_ratios(
                    &Vec2::new(640., 480.),
                    &AspectRatio::try_new(640., 480.)?,
                    &AspectRatio::try_new(640., 360.)?
                ),
                Boxing::new(Vec2::new(0., 60.), Vec2::new(640., 360.))
            );
            
            assert_eq!(
                calculate_boxing_from_aspect_ratios(
                    &Vec2::new(640., 360.),
                    &AspectRatio::try_new(640., 360.)?,
                    &AspectRatio::try_new(640., 480.)?
                ),
                Boxing::new(Vec2::new(80., 0.), Vec2::new(480., 360.))
            );
            
            assert_eq!(
                calculate_boxing_from_aspect_ratios(
                    &Vec2::new(480., 640.),
                    &AspectRatio::try_new(480., 640.)?,
                    &AspectRatio::try_new(1280., 720.)?
                ),
                Boxing::new(Vec2::new(0., 185.), Vec2::new(480., 270.))
            );
            
            assert_eq!(
                calculate_boxing_from_aspect_ratios(
                    &Vec2::new(1280., 720.),
                    &AspectRatio::try_new(1280., 720.)?,
                    &AspectRatio::try_new(480., 640.)?
                ),
                Boxing::new(Vec2::new(370., 0.), Vec2::new(540., 720.))
            );
            
            Ok(())
        }

        #[test]
        fn test_calculate_boxing_imperfect() {
            assert!(
                calculate_boxing_perfect(&Vec2::new(640., 360.), &Vec2::new(640., 360.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against the same resolution failed! (360p -> 360p)",
            );

            // Test Output with Expected Boxing
            assert!(
                calculate_boxing_perfect(&Vec2::new(1920., 1080.), &Vec2::new(1280., 720.))
                    .ok()
                    .flatten()
                    .is_some_and(
                        |u| u == Boxing::new(Vec2::new(320., 180.), Vec2::new(1280., 720.))
                    ),
                "Testing against a non-integer (but square) scaling failed! (720p -> 1080p)"
            );

            // Test Output to perfect scale
            assert!(
                calculate_boxing_perfect(&Vec2::new(3840., 2160.), &Vec2::new(1920., 1080.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against an integer scale resolution failed! (1080p -> 2160p)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(1280., 722.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(0., 1.), Vec2::new(1280., 720.))),
                "Testing against minor increase to height in scaling failed! (360p -> 1280x722)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(1282., 720.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(1., 0.), Vec2::new(1280., 720.))),
                "Testing against minor increase to width in scaling failed! (360p -> 1282x720)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(320., 180.), &Vec2::new(640., 360.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against downscaling failed! (360p -> 180p)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(324., 184.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(2., 2.), Vec2::new(320., 180.))),
                "Testing against slight downscaling failed! (360p -> 180p)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(320., 620.), &Vec2::new(320., 620.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against Vertical Resolutions failed! (320x620 -> 320x620)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(320., 620.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(0., 220.), Vec2::new(320., 180.))),
                "Testing against Vertical Output to Widescreen Input failed! (360p -> 320x620)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(1280., 720.), &Vec2::new(640., 480.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(320., 120.), Vec2::new(640., 480.))),
                "Testing against 4:3 480p -> 16:9 720p failed!"
            );
        }

        #[test]
        fn test_calculate_boxing_perfect() {
            assert!(
                calculate_boxing_perfect(&Vec2::new(640., 360.), &Vec2::new(640., 360.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against the same resolution failed! (360p -> 360p)",
            );

            // Test Output with Expected Boxing
            assert!(
                calculate_boxing_perfect(&Vec2::new(1920., 1080.), &Vec2::new(1280., 720.))
                    .ok()
                    .flatten()
                    .is_some_and(
                        |u| u == Boxing::new(Vec2::new(320., 180.), Vec2::new(1280., 720.))
                    ),
                "Testing against a non-integer (but square) scaling failed! (720p -> 1080p)"
            );

            // Test Output to perfect scale
            assert!(
                calculate_boxing_perfect(&Vec2::new(3840., 2160.), &Vec2::new(1920., 1080.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against an integer scale resolution failed! (1080p -> 2160p)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(1280., 722.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(0., 1.), Vec2::new(1280., 720.))),
                "Testing against minor increase to height in scaling failed! (360p -> 1280x722)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(1282., 720.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(1., 0.), Vec2::new(1280., 720.))),
                "Testing against minor increase to width in scaling failed! (360p -> 1282x720)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(320., 180.), &Vec2::new(640., 360.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against downscaling failed! (360p -> 180p)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(324., 184.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(2., 2.), Vec2::new(320., 180.))),
                "Testing against slight downscaling failed! (360p -> 180p)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(320., 620.), &Vec2::new(320., 620.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against Vertical Resolutions failed! (320x620 -> 320x620)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(320., 620.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(0., 220.), Vec2::new(320., 180.))),
                "Testing against Vertical Output to Widescreen Input failed! (360p -> 320x620)"
            );

            assert!(
                calculate_boxing_perfect(&Vec2::new(1280., 720.), &Vec2::new(640., 480.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(320., 120.), Vec2::new(640., 480.))),
                "Testing against 4:3 480p -> 16:9 720p failed!"
            );
        }

        #[test]
        fn test_calculate_letterbox() {
            let inputs: [(u32, u32); 6] =
                [(100, 100), (100, 0), (100, 50), (50, 100), (0, 0), (0, 100)];
            let physical_size = Vec2::new(640., 360.);
            let outputs: [_; 6] = [
                Boxing::new(Vec2::new(0., 100.), Vec2::new(640., 160.)),
                Boxing::new(Vec2::new(0., 100.), Vec2::new(640., 260.)),
                Boxing::new(Vec2::new(0., 100.), Vec2::new(640., 210.)),
                Boxing::new(Vec2::new(0., 50.), Vec2::new(640., 210.)),
                Boxing::new(Vec2::new(0., 0.), Vec2::new(640., 360.)),
                Boxing::new(Vec2::new(0., 0.), Vec2::new(640., 260.)),
            ];
            for (i, input) in inputs.iter().enumerate() {
                assert_eq!(
                    calculate_letterbox(&physical_size, (&input.0, &input.1)),
                    outputs[i]
                );
            }
        }
        #[test]
        fn test_calculate_pillarbox() {
            let inputs: [(u32, u32); 6] =
                [(100, 100), (100, 0), (100, 50), (50, 100), (0, 0), (0, 100)];
            let physical_size = Vec2::new(640., 360.);
            let outputs = [
                Boxing::new(Vec2::new(100., 0.), Vec2::new(440., 360.)),
                Boxing::new(Vec2::new(100., 0.), Vec2::new(540., 360.)),
                Boxing::new(Vec2::new(100., 0.), Vec2::new(490., 360.)),
                Boxing::new(Vec2::new(50., 0.), Vec2::new(490., 360.)),
                Boxing::new(Vec2::new(0., 0.), Vec2::new(640., 360.)),
                Boxing::new(Vec2::new(0., 0.), Vec2::new(540., 360.)),
            ];
            for (i, input) in inputs.iter().enumerate() {
                assert_eq!(
                    calculate_pillarbox(&physical_size, (&input.0, &input.1)),
                    outputs[i]
                );
            }
        }
    }
}
