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
use bevy_log::{info, warn, warn_once};
use bevy_math::{AspectRatio, UVec2, Vec2};
use bevy_reflect::Reflect;
use bevy_render::camera::{ManualTextureViews, Viewport};
use bevy_render::prelude::*;
use bevy_window::{PrimaryWindow, Window};

/// The Plugin that adds in all the systems for camera-boxing.
pub struct CameraBoxingPlugin;

/// The system set provided and used by the plugin for ordering.
#[derive(SystemSet, Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum CameraBoxSet {
    /// Detect changes that might require us to recalculate boxes.
    /// This runs before RecalculateBoxes
    DetectChanges,

    /// Recalculate the Aspect Ratio Masks/CameraBoxes.
    /// This runs after DetectChanges
    RecalculateBoxes,
}

#[derive(Event)]
/// This event is used to tell us that we need to recalculate our Camera Boxes.
pub struct AdjustBoxing;

impl Plugin for CameraBoxingPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CameraBox>()
            .add_event::<AdjustBoxing>()
            .configure_sets(
                First,
                (
                    CameraBoxSet::DetectChanges.run_if(any_with_component::<CameraBox>),
                    CameraBoxSet::RecalculateBoxes
                        .run_if(on_event::<AdjustBoxing>)
                        .after(CameraBoxSet::DetectChanges),
                ),
            )
            .add_systems(
                First,
                (windows_changed, camerabox_changed).in_set(CameraBoxSet::DetectChanges),
            )
            .add_systems(
                First,
                images_changed.in_set(CameraBoxSet::DetectChanges).run_if(
                    on_event::<AssetEvent<Image>>.or(resource_changed_or_removed::<Assets<Image>>),
                ),
            )
            .add_systems(
                First,
                texture_views_changed
                    .in_set(CameraBoxSet::DetectChanges)
                    .run_if(resource_changed_or_removed::<ManualTextureViews>),
            )
            .add_systems(
                First,
                adjust_viewport.in_set(CameraBoxSet::RecalculateBoxes),
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
        /// If the output is not boxed, this will not be used.
        position: Option<UVec2>,
    },

    /// Keep the output as a static Aspect Ratio. If the output is not at the Aspect Ratio apply
    /// boxing to force it into the correct Aspect Ratio.
    StaticAspectRatio {
        aspect_ratio: AspectRatio,

        /// Where to put the Boxed output, if this is None then it will be centered.
        /// If the output is not boxed, then this will not be used.
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
        allow_imperfect_downscaled_boxing: bool,
    },

    /// Have static letterboxing with specific sizes for each of the bars.
    LetterBox {
        /// The bar at the top of the output.
        top: u32,

        /// The bar at the bottom of the output.
        bottom: u32,

        /// If false, we will attempt to scale the letterboxing if the output is smaller than the
        /// size of the letterboxes. If this is true, then letterboxing will be disabled in the
        /// cases where it would be smaller.
        strict_letterboxing: bool,
    },

    /// Have static Pillarboxing with specific sizes for each of the bars.
    PillarBox {
        /// The bar on the left side of the output.
        left: u32,

        /// The bar on the right side of the output.
        right: u32,

        /// If false, we will attempt to scale the pillarboxing if the output is smaller than the
        /// size of the pillarboxes. If this is true, then pillarboxing will be disabled in the
        /// cases where it would be smaller.
        strict_pillarboxing: bool,
    },

    /// Have static Windowboxing with specific sizes for each of the bars.
    WindowBox {
        /// The bar on the left side of the output.
        left: u32,

        /// The bar on the right side of the output.
        right: u32,

        /// The bar at the top of the output.
        top: u32,

        /// The bar at the bottom of the output.
        bottom: u32,

        /// If false, we will attempt to scale the windowboxing if the output is smaller than the
        /// size of the windowboxes. If this is true, then windowboxing will be disabled in the
        /// cases where it would be smaller.
        strict_windowboxing: bool,
    },
}

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

fn camerabox_changed(
    mut boxing_event: EventWriter<AdjustBoxing>,
    boxes: Query<&CameraBox, Changed<CameraBox>>,
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
        if !camera.is_active {
            continue;
        }
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

        let mut viewport = match &mut camera.viewport {
            None => Viewport::default(),
            Some(viewport) => viewport.to_owned(),
        };
        
        match &camera_box {
            CameraBox::StaticResolution {
                resolution: size,
                position,
            } => {
                if &target.physical_size == size && position.is_none() {
                    camera.viewport = None;
                    continue;
                } else if position.is_some() {
                    let position = position.unwrap();
                    let offset = size.clamp(UVec2::ZERO, target.physical_size) + position;
                    if (target.physical_size.x < offset.x || target.physical_size.y < offset.y)
                        && viewport.physical_position == UVec2::ZERO
                    {
                        continue;
                    }
                }

                if &viewport.physical_size != size {
                    viewport.physical_size = size.clamp(UVec2::ONE, target.physical_size);
                }

                viewport.physical_position = if position.is_none() {
                    (target.physical_size
                        - viewport
                            .physical_size
                            .clamp(UVec2::ZERO, target.physical_size))
                        / 2
                } else {
                    let position = position.unwrap();
                    let offset = size.clamp(UVec2::ZERO, target.physical_size) + position;
                    if target.physical_size.x >= offset.x && target.physical_size.y >= offset.y {
                        position
                    } else {
                        warn_once!(
                            "Unable to place output with resolution {} at position {} within Render Target with size {}. Placing at (0,0) instead",
                            size,
                            position,
                            target.physical_size
                        );
                        UVec2::ZERO
                    }
                };
                camera.viewport = Some(viewport);
            }
            CameraBox::StaticAspectRatio {
                aspect_ratio,
                position,
            } => {
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
                    output_resolution,
                } = calculate_boxing_from_aspect_ratios(
                    &target.physical_size.as_vec2(),
                    &physical_aspect_ratio,
                    aspect_ratio,
                );

                viewport.physical_size = output_resolution.as_uvec2();
                viewport.physical_position = match position {
                    None => boxing_offset.as_uvec2(),
                    Some(pos) => {
                        if is_within_rect(&target.physical_size, pos, &viewport.physical_size) {
                            *pos
                        } else {
                            warn_once!(
                                "Unable to place output with resolution {} at position {} within Render Target with size {}. Placing at (0,0) instead",
                                output_resolution,
                                pos,
                                target.physical_size
                            );
                            UVec2::ZERO
                        }
                    }
                };
                camera.viewport = Some(viewport);
            }

            CameraBox::ResolutionIntegerScale {
                allow_imperfect_downscaled_boxing: allow_imperfect_aspect_ratios,
                resolution,
            } => {
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
                    camera.viewport = None;
                    continue;
                }

                viewport.physical_position = boxing_offset.as_uvec2();
                viewport.physical_size = output_resolution.as_uvec2();
                camera.viewport = Some(viewport);
            }
            CameraBox::PillarBox {
                left,
                right,
                strict_pillarboxing,
            } => {
                let Boxing {
                    mut boxing_offset,
                    mut output_resolution,
                } = calculate_pillarbox(&target.physical_size.as_vec2(), (left, right));

                if (output_resolution.x + boxing_offset.x > target.physical_size.x as f32
                    || output_resolution.x <= 0.)
                    && !strict_pillarboxing
                {
                    output_resolution.x = target.physical_size.x as f32 / 2.;
                    boxing_offset.x /= 2.;
                    let scale_factor =
                        (target.physical_size.x as f32) / (output_resolution.x + boxing_offset.x);
                    boxing_offset.x *= scale_factor;
                }

                if output_resolution.x <= 0.
                    || output_resolution.x > target.physical_size.x as f32
                    || output_resolution.x + boxing_offset.x > target.physical_size.x as f32
                        && *strict_pillarboxing
                {
                    camera.viewport = None;
                    continue;
                }

                viewport.physical_position = boxing_offset.as_uvec2();
                viewport.physical_size = output_resolution.as_uvec2();
                camera.viewport = Some(viewport);
            }
            CameraBox::WindowBox {
                left,
                right,
                top,
                bottom,
                strict_windowboxing,
            } => {
                let letterboxing = (top, bottom);
                let pillarboxing = (left, right);

                let Boxing {
                    mut boxing_offset,
                    mut output_resolution,
                } = calculate_windowbox(
                    &target.physical_size.as_vec2(),
                    [letterboxing, pillarboxing],
                );

                if *strict_windowboxing {
                    if output_resolution.x <= 0.
                        || !is_within_rect(
                            &target.physical_size,
                            &boxing_offset.as_uvec2(),
                            &output_resolution.as_uvec2(),
                        )
                    {
                        camera.viewport = None;
                        continue;
                    }
                } else {
                    if output_resolution.x + boxing_offset.x > target.physical_size.x as f32
                        || output_resolution.x <= 0.
                    {
                        output_resolution.x = target.physical_size.x as f32 / 2.;
                        boxing_offset.x /= 2.;
                        let scale_factor = (target.physical_size.x as f32)
                            / (output_resolution.x + boxing_offset.x);
                        boxing_offset.x *= scale_factor;
                    }

                    if output_resolution.y + boxing_offset.y > target.physical_size.y as f32
                        || output_resolution.y <= 0.
                    {
                        output_resolution.y = target.physical_size.y as f32 / 2.;
                        boxing_offset.y /= 2.;
                        let scale_factor = (target.physical_size.y as f32)
                            / (output_resolution.y + boxing_offset.y);
                        boxing_offset.y *= scale_factor;
                    }
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

fn calculate_windowbox(physical_size: &Vec2, windowbox: [(&u32, &u32); 2]) -> Boxing {
    let letterbox_height = (windowbox[0].0 + windowbox[0].1) as f32;
    let pillarbox_width = (windowbox[1].0 + windowbox[1].1) as f32;

    let render_height = physical_size.y - letterbox_height;
    let render_width = physical_size.x - pillarbox_width;

    Boxing {
        boxing_offset: Vec2::new(*windowbox[1].0 as f32, *windowbox[0].0 as f32),
        output_resolution: Vec2::new(render_width, render_height),
    }
}

fn is_within_rect(rect: &UVec2, position: &UVec2, size: &UVec2) -> bool {
    let actual_bounds = position + size;
    rect.x >= actual_bounds.x && rect.y >= actual_bounds.y
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
                calculate_boxing_imperfect(&Vec2::new(640., 360.), &Vec2::new(640., 360.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against the same resolution failed! (360p -> 360p)",
            );

            // Test Output with Expected Boxing
            assert!(
                calculate_boxing_imperfect(&Vec2::new(1920., 1080.), &Vec2::new(1280., 720.))
                    .ok()
                    .flatten()
                    .is_some_and(
                        |u| u == Boxing::new(Vec2::new(320., 180.), Vec2::new(1280., 720.))
                    ),
                "Testing against a non-integer (but square) scaling failed! (720p -> 1080p)"
            );

            // Test Output to imperfect scale
            assert!(
                calculate_boxing_imperfect(&Vec2::new(3840., 2160.), &Vec2::new(1920., 1080.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against an integer scale resolution failed! (1080p -> 2160p)"
            );

            assert!(
                calculate_boxing_imperfect(&Vec2::new(1280., 722.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(0., 1.), Vec2::new(1280., 720.))),
                "Testing against minor increase to height in scaling failed! (360p -> 1280x722)"
            );

            assert!(
                calculate_boxing_imperfect(&Vec2::new(1282., 720.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(1., 0.), Vec2::new(1280., 720.))),
                "Testing against minor increase to width in scaling failed! (360p -> 1282x720)"
            );

            assert!(
                calculate_boxing_imperfect(&Vec2::new(320., 180.), &Vec2::new(640., 360.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against downscaling failed! (360p -> 180p)"
            );

            assert!(
                calculate_boxing_imperfect(&Vec2::new(330., 190.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(
                        |u| u == Boxing::new(Vec2::new(0., 2.1875), Vec2::new(330., 185.625))
                    ),
                "Testing against off downscaling failed! (360p -> (180p + 10))"
            );

            assert!(
                calculate_boxing_imperfect(&Vec2::new(320., 620.), &Vec2::new(320., 620.))
                    .is_ok_and(|u| u.is_none()),
                "Testing against Vertical Resolutions failed! (320x620 -> 320x620)"
            );

            assert!(
                calculate_boxing_imperfect(&Vec2::new(320., 620.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(0., 220.), Vec2::new(320., 180.))),
                "Testing against Vertical Output to Widescreen Input failed! (360p -> 320x620)"
            );

            assert!(
                calculate_boxing_imperfect(&Vec2::new(1280., 720.), &Vec2::new(640., 480.))
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
                calculate_boxing_perfect(&Vec2::new(330., 190.), &Vec2::new(640., 360.))
                    .ok()
                    .flatten()
                    .is_some_and(|u| u == Boxing::new(Vec2::new(5., 5.), Vec2::new(320., 180.))),
                "Testing against off downscaling failed! (360p -> (180p + 10))"
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

        #[test]
        fn test_calculate_windowbox() {
            let inputs: [[(&u32, &u32); 2]; 8] = [
                [(&0, &0), (&0, &0)],     //Test Noboxing
                [(&100, &100), (&0, &0)], //Test Letterboxing
                [(&0, &0), (&100, &100)], //Test Pillarboxing
                [(&50, &0), (&50, &0)],   //Test Boxing Bottom Left
                [(&0, &50), (&0, &50)],   //Test Bottom Boxing.
                [(&50, &50), (&50, &50)], //Test Full Boxing
                [(&50, &0), (&0, &50)],   //Test Opp Boxing
                [(&0, &50), (&50, &0)],   //Test Opp Boxing 2
            ];
            let physical_size = Vec2::new(640., 360.);

            let outputs: [Boxing; 8] = [
                Boxing::new(Vec2::new(0., 0.), physical_size),
                Boxing::new(Vec2::new(0., 100.), Vec2::new(640., 160.)),
                Boxing::new(Vec2::new(100., 0.), Vec2::new(440., 360.)),
                Boxing::new(Vec2::new(50., 50.), Vec2::new(590., 310.)),
                Boxing::new(Vec2::new(0., 0.), Vec2::new(590., 310.)),
                Boxing::new(Vec2::new(50., 50.), Vec2::new(540., 260.)),
                Boxing::new(Vec2::new(0., 50.), Vec2::new(590., 310.)),
                Boxing::new(Vec2::new(50., 0.), Vec2::new(590., 310.)),
            ];

            for (i, input) in inputs.into_iter().enumerate() {
                assert_eq!(calculate_windowbox(&physical_size, input), outputs[i],);
            }
        }
    }

    mod systems {
        use super::*;
        use bevy_asset::AssetId;
        use bevy_render::camera::RenderTarget;
        use bevy_window::{WindowRef, WindowResolution};

        const W360P: UVec2 = UVec2::new(640, 360);
        const W720P: UVec2 = UVec2::new(1280, 720);
        const W180P: UVec2 = UVec2::new(320, 180);

        fn setup_app(camerabox: CameraBox, window_resolution: WindowResolution) -> (App, Entity) {
            let mut app = App::new();

            app.init_resource::<ManualTextureViews>();
            app.init_resource::<Assets<Image>>();
            app.world_mut().spawn((
                Window {
                    resolution: window_resolution,
                    ..Window::default()
                },
                PrimaryWindow,
            ));
            let camera_id = app
                .world_mut()
                .spawn((
                    Camera {
                        viewport: None,
                        is_active: true,
                        target: RenderTarget::Window(WindowRef::Primary),
                        ..Camera::default()
                    },
                    camerabox,
                ))
                .id();
            app.add_systems(First, adjust_viewport);
            (app, camera_id)
        }

        #[test]
        fn test_basic_windowboxing() {
            let (mut app, camera_id) = setup_app(
                CameraBox::WindowBox {
                    left: 10,
                    right: 10,
                    top: 10,
                    bottom: 10,
                    strict_windowboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(10, 10));
            assert_eq!(viewport.physical_size, UVec2::new(620, 340));

            let (mut app, camera_id) = setup_app(
                CameraBox::WindowBox {
                    left: 10,
                    right: 10,
                    top: 10,
                    bottom: 10,
                    strict_windowboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(10, 10));
            assert_eq!(viewport.physical_size, UVec2::new(620, 340));

            let (mut app, camera_id) = setup_app(
                CameraBox::WindowBox {
                    left: 650,
                    right: 0,
                    top: 370,
                    bottom: 0,
                    strict_windowboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let (mut app, camera_id) = setup_app(
                CameraBox::WindowBox {
                    left: 650,
                    right: 0,
                    top: 370,
                    bottom: 0,
                    strict_windowboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(322, 182));
            assert_eq!(viewport.physical_size, UVec2::new(320, 180));
        }

        #[test]
        fn test_basic_pillarboxing() {
            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 2,
                    right: 2,
                    strict_pillarboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(2, 0));
            assert_eq!(viewport.physical_size, UVec2::new(636, 360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 5,
                    right: 0,
                    strict_pillarboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(5, 0));
            assert_eq!(viewport.physical_size, UVec2::new(635, 360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 0,
                    right: 5,
                    strict_pillarboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 0));
            assert_eq!(viewport.physical_size, UVec2::new(635, 360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 5,
                    right: 10,
                    strict_pillarboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(5, 0));
            assert_eq!(viewport.physical_size, UVec2::new(625, 360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 10,
                    right: 5,
                    strict_pillarboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(10, 0));
            assert_eq!(viewport.physical_size, UVec2::new(625, 360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 640,
                    right: 0,
                    strict_pillarboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(320, 0));
            assert_eq!(viewport.physical_size, UVec2::from(W180P).with_y(360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 2,
                    right: 2,
                    strict_pillarboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(2, 0));
            assert_eq!(viewport.physical_size, UVec2::new(636, 360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 5,
                    right: 0,
                    strict_pillarboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(5, 0));
            assert_eq!(viewport.physical_size, UVec2::new(635, 360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 0,
                    right: 5,
                    strict_pillarboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 0));
            assert_eq!(viewport.physical_size, UVec2::new(635, 360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 5,
                    right: 10,
                    strict_pillarboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(5, 0));
            assert_eq!(viewport.physical_size, UVec2::new(625, 360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 10,
                    right: 5,
                    strict_pillarboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(10, 0));
            assert_eq!(viewport.physical_size, UVec2::new(625, 360));

            let (mut app, camera_id) = setup_app(
                CameraBox::PillarBox {
                    left: 640,
                    right: 0,
                    strict_pillarboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());
        }

        #[test]
        fn test_basic_letterboxing() {
            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 2,
                    bottom: 2,
                    strict_letterboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 2));
            assert_eq!(viewport.physical_size, UVec2::new(640, 356));

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 5,
                    bottom: 0,
                    strict_letterboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 5));
            assert_eq!(viewport.physical_size, UVec2::new(640, 355));

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 0,
                    bottom: 5,
                    strict_letterboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 0));
            assert_eq!(viewport.physical_size, UVec2::new(640, 355));

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 10,
                    bottom: 5,
                    strict_letterboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 10));
            assert_eq!(viewport.physical_size, UVec2::new(640, 345));

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 5,
                    bottom: 10,
                    strict_letterboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 5));
            assert_eq!(viewport.physical_size, UVec2::new(640, 345));

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 360,
                    bottom: 0,
                    strict_letterboxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 2,
                    bottom: 2,
                    strict_letterboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 2));
            assert_eq!(viewport.physical_size, UVec2::new(640, 356));

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 5,
                    bottom: 0,
                    strict_letterboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 5));
            assert_eq!(viewport.physical_size, UVec2::new(640, 355));

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 0,
                    bottom: 5,
                    strict_letterboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 0));
            assert_eq!(viewport.physical_size, UVec2::new(640, 355));

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 10,
                    bottom: 5,
                    strict_letterboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 10));
            assert_eq!(viewport.physical_size, UVec2::new(640, 345));

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 5,
                    bottom: 10,
                    strict_letterboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 5));
            assert_eq!(viewport.physical_size, UVec2::new(640, 345));

            let (mut app, camera_id) = setup_app(
                CameraBox::LetterBox {
                    top: 360,
                    bottom: 0,
                    strict_letterboxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 180));
            assert_eq!(viewport.physical_size, UVec2::new(640, 180));
        }

        #[test]
        fn test_basic_resolution() {
            let (mut app, camera_id) = setup_app(
                CameraBox::StaticResolution {
                    resolution: W360P.into(),
                    position: None,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let (mut app, camera_id) = setup_app(
                CameraBox::StaticResolution {
                    resolution: W360P.into(),
                    position: Some((1, 0).into()),
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let (mut app, camera_id) = setup_app(
                CameraBox::StaticResolution {
                    resolution: W360P.into(),
                    position: None,
                },
                W720P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(320, 180));
            assert_eq!(viewport.physical_size, W360P);

            let (mut app, camera_id) = setup_app(
                CameraBox::StaticResolution {
                    resolution: W360P.into(),
                    position: None,
                },
                W180P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 0));
            assert_eq!(viewport.physical_size, W180P);
        }

        #[test]
        fn test_basic_aspect_ratio() -> Result<()> {
            let desired_aspect_ratio = AspectRatio::try_from(W720P.as_vec2())?;
            let (mut app, camera_id) = setup_app(
                CameraBox::StaticAspectRatio {
                    aspect_ratio: desired_aspect_ratio,
                    position: None,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let desired_aspect_ratio = AspectRatio::try_new(640., 480.)?;
            let (mut app, camera_id) = setup_app(
                CameraBox::StaticAspectRatio {
                    aspect_ratio: desired_aspect_ratio,
                    position: None,
                },
                W720P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(160, 0));
            assert_eq!(viewport.physical_size, UVec2::new(960, 720));

            let desired_aspect_ratio = AspectRatio::try_from(W720P.as_vec2())?;
            let (mut app, camera_id) = setup_app(
                CameraBox::StaticAspectRatio {
                    aspect_ratio: desired_aspect_ratio,
                    position: Some((1, 0).into()),
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            Ok(())
        }

        #[test]
        fn test_basic_integer_scaling_imperfect() {
            let (mut app, camera_id) = setup_app(
                CameraBox::ResolutionIntegerScale {
                    resolution: W360P.as_vec2().into(),
                    allow_imperfect_downscaled_boxing: true,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let (mut app, camera_id) = setup_app(
                CameraBox::ResolutionIntegerScale {
                    resolution: (640., 480.).into(),
                    allow_imperfect_downscaled_boxing: true,
                },
                W720P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(320, 120));
            assert_eq!(viewport.physical_size, UVec2::new(640, 480));

            let (mut app, camera_id) = setup_app(
                CameraBox::ResolutionIntegerScale {
                    resolution: W360P.as_vec2(),
                    allow_imperfect_downscaled_boxing: true,
                },
                W720P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let (mut app, camera_id) = setup_app(
                CameraBox::ResolutionIntegerScale {
                    resolution: W360P.as_vec2().into(),
                    allow_imperfect_downscaled_boxing: true,
                },
                W180P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let (mut app, camera_id) = setup_app(
                CameraBox::ResolutionIntegerScale {
                    resolution: W360P.as_vec2().into(),
                    allow_imperfect_downscaled_boxing: true,
                },
                (W180P + 10).as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(0, 2));
            assert_eq!(viewport.physical_size, UVec2::new(330, 185));
        }

        #[test]
        fn test_basic_integer_scaling_perfect() {
            let (mut app, camera_id) = setup_app(
                CameraBox::ResolutionIntegerScale {
                    resolution: W360P.as_vec2().into(),
                    allow_imperfect_downscaled_boxing: false,
                },
                W360P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let (mut app, camera_id) = setup_app(
                CameraBox::ResolutionIntegerScale {
                    resolution: (640., 480.).into(),
                    allow_imperfect_downscaled_boxing: false,
                },
                W720P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(320, 120));
            assert_eq!(viewport.physical_size, UVec2::new(640, 480));

            let (mut app, camera_id) = setup_app(
                CameraBox::ResolutionIntegerScale {
                    resolution: W360P.as_vec2(),
                    allow_imperfect_downscaled_boxing: false,
                },
                W720P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let (mut app, camera_id) = setup_app(
                CameraBox::ResolutionIntegerScale {
                    resolution: W360P.as_vec2().into(),
                    allow_imperfect_downscaled_boxing: false,
                },
                W180P.as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport;
            assert!(viewport.is_none());

            let (mut app, camera_id) = setup_app(
                CameraBox::ResolutionIntegerScale {
                    resolution: W360P.as_vec2().into(),
                    allow_imperfect_downscaled_boxing: false,
                },
                (W180P + 10).as_vec2().into(),
            );
            app.update();
            let viewport = app
                .world()
                .get::<Camera>(camera_id)
                .unwrap()
                .to_owned()
                .viewport
                .unwrap();
            assert_eq!(viewport.physical_position, UVec2::new(5, 5));
            assert_eq!(viewport.physical_size, UVec2::new(320, 180));
        }

        #[test]
        fn test_camerabox_changed_detection() {
            let mut app = App::new();

            app.init_resource::<ManualTextureViews>();
            app.init_resource::<Assets<Image>>();
            app.world_mut().spawn((
                Window {
                    resolution: W360P.as_vec2().into(),
                    ..Window::default()
                },
                PrimaryWindow,
            ));
            let camera_id = app
                .world_mut()
                .spawn((
                    Camera {
                        viewport: None,
                        is_active: true,
                        target: RenderTarget::Window(WindowRef::Primary),
                        ..Camera::default()
                    },
                    CameraBox::StaticResolution {
                        resolution: W360P,
                        position: None,
                    },
                ))
                .id();
            app.add_systems(
                First,
                camerabox_changed.run_if(any_with_component::<CameraBox>),
            );
            app.add_event::<AdjustBoxing>();
            app.update();
            let mut camera_box = app.world_mut().get_mut::<CameraBox>(camera_id).unwrap();
            *camera_box = CameraBox::LetterBox {
                top: 10,
                bottom: 10,
                strict_letterboxing: true,
            };
            app.update();
            let adjust_boxing_events = app.world().resource::<Events<AdjustBoxing>>();
            let mut adjust_boxing_reader = adjust_boxing_events.get_cursor();
            let boxing_adjust = adjust_boxing_reader.read(adjust_boxing_events).next();

            assert!(boxing_adjust.is_some())
        }

        #[test]
        fn test_window_changed_detection() {
            let mut app = App::new();

            app.init_resource::<ManualTextureViews>();
            app.init_resource::<Assets<Image>>();
            let window_id = app
                .world_mut()
                .spawn((
                    Window {
                        resolution: W360P.as_vec2().into(),
                        ..Window::default()
                    },
                    PrimaryWindow,
                ))
                .id();
            app.world_mut().spawn((CameraBox::StaticResolution {
                resolution: W360P,
                position: None,
            },));
            app.add_systems(
                First,
                windows_changed.run_if(any_with_component::<CameraBox>),
            );
            app.add_event::<AdjustBoxing>();
            app.update();
            let mut window = app.world_mut().get_mut::<Window>(window_id).unwrap();
            window.resolution = W720P.as_vec2().into();
            app.update();
            let adjust_boxing_events = app.world().resource::<Events<AdjustBoxing>>();
            let mut adjust_boxing_reader = adjust_boxing_events.get_cursor();
            let boxing_adjust = adjust_boxing_reader.read(adjust_boxing_events).next();

            assert!(boxing_adjust.is_some())
        }

        #[test]
        fn test_image_changed_detection() {
            let mut app = App::new();

            app.init_resource::<ManualTextureViews>();
            app.init_resource::<Assets<Image>>();
            app.add_event::<AssetEvent<Image>>();
            app.add_event::<AdjustBoxing>();
            app.add_systems(
                First,
                images_changed.run_if(any_with_component::<CameraBox>.and(
                    resource_changed_or_removed::<Assets<Image>>.or(on_event::<AssetEvent<Image>>),
                )),
            );
            app.update();

            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            images.add(Image::default());
            app.update();
            let adjust_boxing_events = app.world().resource::<Events<AdjustBoxing>>();
            let mut adjust_boxing_reader = adjust_boxing_events.get_cursor();
            let boxing_adjust = adjust_boxing_reader.read(adjust_boxing_events).next();
            assert!(boxing_adjust.is_none());

            let event = AssetEvent::Modified {
                id: AssetId::default(),
            };
            app.world_mut().send_event::<AssetEvent<Image>>(event);
            app.update();
            let adjust_boxing_events = app.world().resource::<Events<AdjustBoxing>>();
            let mut adjust_boxing_reader = adjust_boxing_events.get_cursor();
            let boxing_adjust = adjust_boxing_reader.read(adjust_boxing_events).next();
            assert!(boxing_adjust.is_none());

            app.world_mut().spawn(CameraBox::LetterBox {
                top: 0,
                bottom: 0,
                strict_letterboxing: true,
            });
            app.update();

            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            images.add(Image::default());
            app.update();
            let adjust_boxing_events = app.world().resource::<Events<AdjustBoxing>>();
            let mut adjust_boxing_reader = adjust_boxing_events.get_cursor();
            let boxing_adjust = adjust_boxing_reader.read(adjust_boxing_events).next();
            assert!(boxing_adjust.is_some());

            let event = AssetEvent::Modified {
                id: AssetId::default(),
            };
            app.world_mut().send_event::<AssetEvent<Image>>(event);
            app.update();
            let adjust_boxing_events = app.world().resource::<Events<AdjustBoxing>>();
            let mut adjust_boxing_reader = adjust_boxing_events.get_cursor();
            let boxing_adjust = adjust_boxing_reader.read(adjust_boxing_events).next();
            assert!(boxing_adjust.is_some());
            app.update();

            let adjust_boxing_events = app.world().resource::<Events<AdjustBoxing>>();
            let mut adjust_boxing_reader = adjust_boxing_events.get_cursor();
            let boxing_adjust = adjust_boxing_reader.read(adjust_boxing_events).next();
            assert!(boxing_adjust.is_none());
        }

        #[test]
        fn test_textureviews_changed_detection() {
            let mut app = App::new();

            app.init_resource::<ManualTextureViews>();
            app.init_resource::<Assets<Image>>();
            app.add_event::<AdjustBoxing>();
            app.update();
            app.add_systems(
                First,
                texture_views_changed.run_if(
                    any_with_component::<CameraBox>
                        .and(resource_changed_or_removed::<ManualTextureViews>),
                ),
            );

            // While this doesn't actually change anything it *does* work by forcing the Bevy
            // to detect a change, even though we don't do anything, since Bevy has to assume that
            // any mutable access might've changed something, it seems.
            let _ = app.world_mut().resource_mut::<ManualTextureViews>();
            app.update();
            let adjust_boxing_events = app.world().resource::<Events<AdjustBoxing>>();
            let mut adjust_boxing_reader = adjust_boxing_events.get_cursor();
            let boxing_adjust = adjust_boxing_reader.read(adjust_boxing_events).next();
            assert!(boxing_adjust.is_none());

            app.world_mut().spawn(CameraBox::LetterBox {
                top: 0,
                bottom: 0,
                strict_letterboxing: false,
            });

            let _ = app.world_mut().resource_mut::<ManualTextureViews>();
            app.update();
            let adjust_boxing_events = app.world().resource::<Events<AdjustBoxing>>();
            let mut adjust_boxing_reader = adjust_boxing_events.get_cursor();
            let boxing_adjust = adjust_boxing_reader.read(adjust_boxing_events).next();
            assert!(boxing_adjust.is_some());
        }
    }
}
