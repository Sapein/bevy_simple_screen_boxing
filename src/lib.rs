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
use bevy_math::{AspectRatio, UVec2, Vec2};
use bevy_reflect::Reflect;
use bevy_render::camera::{ManualTextureViews, Viewport};
use bevy_render::prelude::*;
use bevy_utils::default;
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
    /// If the output is smaller than the resolution, it will output at the smaller resolution instead.
    StaticResolution {
        resolution: UVec2,

        /// Where to put the Boxed output, if this is None then it will be centered.
        position: Option<UVec2>,
    },

    /// Keep the output as a static Aspect Ratio. If the output is not at the Aspect Ratio
    /// apply boxing to force it into the correct Aspect Ratio.
    StaticAspectRatio {
        aspect_ratio: AspectRatio,

        /// Where to put the Boxed output, if this is None then it will be centered.
        position: Option<UVec2>,
    },

    /// Keep the output at an Integer Scale of a specific Resolution, if no Integer Scale exists
    /// box the output to an Integer Scale.
    ResolutionIntegerScale {
        resolution: Vec2,

        /// If this is true, then the output may not be *exactly* the proper Aspect Ratio (being off
        /// by at most ~0.0001), especially when scaling down. If this is false, then we will do
        /// whatever we can to maintain the Aspect Ratio, no matter what. Although it might still be
        /// off but a small amount (about ~0.00001)
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

fn camerabox_changed(
    mut boxing_event: EventWriter<AdjustBoxing>,
    boxes: Query<&CameraBox, Or<(Changed<CameraBox>, Changed<Camera>)>>,
) {
    if !boxes.is_empty() {
        boxing_event.write(AdjustBoxing);
    }
}

fn adjust_viewport(
    mut boxed_cameras: Query<(&mut Camera, &CameraBox)>,
    primary_window: Query<Option<Entity>, With<PrimaryWindow>>,
    windows: Query<(Entity, &Window)>,
    texture_views: Res<ManualTextureViews>,
    images: Res<Assets<Image>>,
) {
    for (mut camera, camera_box) in boxed_cameras.iter_mut() {
        let target = camera.target.normalize(
            primary_window
                .iter()
                .collect::<Vec<Option<Entity>>>()
                .first()
                .unwrap()
                .to_owned(),
        ); // Probably a better way to do this.

        let target =
            match target.and_then(|t| t.get_render_target_info(windows, &images, &texture_views)) {
                None => continue,
                Some(target) => target,
            };

        match &camera_box {
            CameraBox::StaticResolution {
                resolution: size,
                position,
            } => match &mut camera.viewport {
                Some(viewport) => {
                    if &viewport.physical_size != size {
                        if size.x > viewport.physical_size.x || size.y > viewport.physical_size.y {
                            viewport.physical_size = *size;
                            if target.physical_size.x < size.x {
                                viewport.physical_size.x = target.physical_size.x;
                            }
                            if target.physical_size.y < size.y {
                                viewport.physical_size.y = target.physical_size.y;
                            }
                        } else {
                            viewport.clamp_to_size(*size);
                        }
                    }
                    if position.is_some_and(|u| u != viewport.physical_position) {
                        viewport.physical_position = position.unwrap();
                    } else if position.is_none() {
                        viewport.physical_position = if (size.x < target.physical_size.x)
                            && (size.y < target.physical_size.y)
                        {
                            (target.physical_size - viewport.physical_size) / 2
                        } else {
                            UVec2::ZERO
                        }
                    }
                }
                None => {
                    camera.viewport = Some(Viewport {
                        physical_size: if (target.physical_size.x < size.x)
                            || (target.physical_size.y < size.y)
                        {
                            *size
                        } else {
                            target.physical_size
                        },
                        physical_position: match position {
                            Some(pos) => *pos,
                            None => {
                                if (size.x < target.physical_size.x)
                                    && (size.y < target.physical_size.y)
                                {
                                    (target.physical_size - size) / 2
                                } else {
                                    UVec2::ZERO
                                }
                            }
                        },
                        ..default()
                    })
                }
            },
            CameraBox::StaticAspectRatio {
                aspect_ratio,
                position,
            } => match &mut camera.viewport {
                None => {
                    let physical_aspect_ratio =
                        match AspectRatio::try_from(target.physical_size.as_vec2()) {
                            Ok(ar) => ar,
                            Err(_) => continue,
                        };
                    if physical_aspect_ratio.ratio() == aspect_ratio.ratio() {
                        camera.viewport = None;
                        continue;
                    }
                    let (boxing, sizing) =
                        calculate_sizes_resolution(&target.physical_size.as_vec2(), aspect_ratio)
                            .unwrap();
                    camera.viewport = Some(Viewport {
                        physical_position: boxing.as_uvec2(),
                        physical_size: match position {
                            None => sizing.as_uvec2(),
                            Some(pos) => *pos,
                        },
                        ..default()
                    });
                }
                Some(viewport) => {
                    let physical_aspect_ratio =
                        match AspectRatio::try_from(target.physical_size.as_vec2()) {
                            Ok(ar) => ar,
                            Err(_) => continue,
                        };
                    if physical_aspect_ratio.ratio() == aspect_ratio.ratio() {
                        camera.viewport = None;
                        continue;
                    }
                    let (boxing, sizing) =
                        calculate_sizes_resolution(&target.physical_size.as_vec2(), aspect_ratio)
                            .unwrap();
                    viewport.physical_position = boxing.as_uvec2();
                    viewport.physical_size = match position {
                        None => sizing.as_uvec2(),
                        Some(pos) => *pos,
                    }
                }
            },
            CameraBox::ResolutionIntegerScale {
                allow_imperfect_aspect_ratios,
                resolution,
            } => {
                let (boxing, sizing) = if *allow_imperfect_aspect_ratios {
                    match calculate_sizes_imperfect(&target.physical_size.as_vec2(), resolution) {
                        Ok(opt) => match opt {
                            None => {
                                camera.viewport = None;
                                continue;
                            }
                            Some(t) => t,
                        },
                        Err(_) => continue,
                    }
                } else {
                    match calculate_sizes_perfect(&target.physical_size.as_vec2(), resolution) {
                        Ok(opt) => match opt {
                            None => {
                                camera.viewport = None;
                                continue;
                            }
                            Some(t) => t,
                        },
                        Err(_) => continue,
                    }
                };

                camera.viewport = Some(Viewport {
                    physical_position: boxing.as_uvec2(),
                    physical_size: sizing.as_uvec2(),
                    ..default()
                });
            }
            CameraBox::LetterBox {
                top,
                bottom,
                strict_letterboxing,
            } => match &mut camera.viewport {
                None => {
                    let (mut boxing, mut sizing) = calculate_aspect_ratio_from_letterbox(
                        &target.physical_size.as_vec2(),
                        (top, bottom),
                    )
                    .unwrap();
                    if (sizing.y + boxing.y > target.physical_size.y as f32 || sizing.y <= 0.)
                        && !strict_letterboxing
                    {
                        sizing.y = target.physical_size.y as f32 / 2.;
                        boxing.y /= 2.;
                        let scale_factor = (target.physical_size.y as f32) / (sizing.y + boxing.y);
                        boxing.y *= scale_factor;
                    }

                    if (sizing.y <= 0.
                        || sizing.y > target.physical_size.y as f32
                        || sizing.y + boxing.y > target.physical_size.y as f32)
                        && *strict_letterboxing
                    {
                        sizing.y = target.physical_size.y as f32;
                        sizing.x = target.physical_size.x as f32;
                        boxing.y = 0.;
                    }

                    camera.viewport = Some(Viewport {
                        physical_position: boxing.as_uvec2(),
                        physical_size: sizing.as_uvec2(),
                        ..default()
                    });
                }
                Some(viewport) => {
                    let (mut boxing, mut sizing) = calculate_aspect_ratio_from_letterbox(
                        &target.physical_size.as_vec2(),
                        (top, bottom),
                    )
                    .unwrap();

                    if (sizing.y + boxing.y > target.physical_size.y as f32 || sizing.y <= 0.)
                        && !strict_letterboxing
                    {
                        sizing.y = target.physical_size.y as f32 / 2.;
                        boxing.y /= 2.;
                        let scale_factor = (target.physical_size.y as f32) / (sizing.y + boxing.y);
                        boxing.y *= scale_factor;
                    }

                    if (sizing.y <= 0.
                        || sizing.y > target.physical_size.y as f32
                        || sizing.y + boxing.y > target.physical_size.y as f32)
                        && *strict_letterboxing
                    {
                        sizing.y = target.physical_size.y as f32;
                        sizing.x = target.physical_size.x as f32;
                        boxing.y = 0.;
                    }

                    viewport.physical_position = boxing.as_uvec2();
                    viewport.physical_size = sizing.as_uvec2();
                }
            },
            CameraBox::PillarBox { left, right } => match &mut camera.viewport {
                None => {
                    let (mut boxing, mut sizing) = calculate_aspect_ratio_from_pillarbox(
                        &target.physical_size.as_vec2(),
                        (left, right),
                    )
                    .unwrap();

                    if sizing.x <= 0.
                        || sizing.x > target.physical_size.x as f32
                        || sizing.x + boxing.x > target.physical_size.x as f32
                    {
                        sizing.x = target.physical_size.x as f32;
                        sizing.x = target.physical_size.x as f32;
                        boxing.x = 0.;
                    }

                    camera.viewport = Some(Viewport {
                        physical_position: boxing.as_uvec2(),
                        physical_size: sizing.as_uvec2(),
                        ..default()
                    });
                }
                Some(viewport) => {
                    let (mut boxing, mut sizing) = calculate_aspect_ratio_from_pillarbox(
                        &target.physical_size.as_vec2(),
                        (left, right),
                    )
                    .unwrap();

                    if sizing.x <= 0.
                        || sizing.x > target.physical_size.x as f32
                        || sizing.x + boxing.x > target.physical_size.x as f32
                    {
                        sizing.x = target.physical_size.x as f32;
                        sizing.x = target.physical_size.x as f32;
                        boxing.x = 0.;
                    }

                    viewport.physical_position = boxing.as_uvec2();
                    viewport.physical_size = sizing.as_uvec2();
                }
            },
        }
    }
}

fn calculate_sizes_resolution(
    physical_size: &Vec2,
    target_aspect_ratio: &AspectRatio,
) -> Option<(Vec2, Vec2)> {
    let physical_aspect_ratio = AspectRatio::try_from(*physical_size);
    if physical_aspect_ratio.is_err() {
        return None;
    }
    let physical_aspect_ratio = physical_aspect_ratio.unwrap();

    if physical_aspect_ratio.ratio() > target_aspect_ratio.ratio() {
        let render_height = physical_size.y;
        let render_width = render_height * target_aspect_ratio.ratio();
        Some((
            Vec2::new(physical_size.x / 2. - render_width / 2., 0.),
            Vec2::new(render_width, render_height),
        ))
    } else {
        let render_width = physical_size.x;
        let render_height = render_width / target_aspect_ratio.ratio();
        Some((
            Vec2::new(0., physical_size.y / 2. - render_height / 2.),
            Vec2::new(render_width, render_height),
        ))
    }
}
fn calculate_sizes_imperfect(
    physical_size: &Vec2,
    desired_size: &Vec2,
) -> Result<Option<(Vec2, Vec2)>, ()> {
    let desired_aspect_ratio = AspectRatio::try_from(*desired_size);
    let physical_aspect_ratio = AspectRatio::try_from(*physical_size);
    if desired_aspect_ratio.is_err() || physical_aspect_ratio.is_err() {
        return Err(());
    }

    let desired_ar = desired_aspect_ratio.unwrap();
    let physical_ar = physical_aspect_ratio.unwrap();

    //NOTE: this does not really handle the case where the target size is smaller than the desired height/width.
    let height_scale = physical_size.y / desired_size.y;
    let width_scale = physical_size.x / desired_size.x;

    let small_height_scale = desired_size.y / physical_size.y;
    let small_width_scale = desired_size.x / physical_size.x;

    let has_int_scale = desired_ar.ratio() == physical_ar.ratio()
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

    Ok(Some((
        Vec2::new(pillarbox_size / 2., letterbox_size / 2.),
        Vec2::new(render_width, render_height),
    )))
}
fn calculate_sizes_perfect(
    physical_size: &Vec2,
    desired_size: &Vec2,
) -> Result<Option<(Vec2, Vec2)>, ()> {
    let desired_aspect_ratio = AspectRatio::try_from(*desired_size);
    let physical_aspect_ratio = AspectRatio::try_from(*physical_size);
    if desired_aspect_ratio.is_err() || physical_aspect_ratio.is_err() {
        return Err(());
    }

    let desired_ar = desired_aspect_ratio.unwrap();
    let physical_ar = physical_aspect_ratio.unwrap();

    let height_scale = physical_size.y / desired_size.y;
    let width_scale = physical_size.x / desired_size.x;

    let has_int_scale = desired_ar.ratio() == physical_ar.ratio()
        && (height_scale % 1. == 0. && width_scale % 1. == 0.);

    // Integer Scaling Exists
    if has_int_scale {
        return Ok(None);
    }

    if height_scale < 1. || width_scale < 1. {
        let height_scale = desired_size.y / physical_size.y;
        let width_scale = desired_size.x / physical_size.x;

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
        Ok(Some((
            Vec2::new(pillarbox_size / 2., letterbox_size / 2.),
            Vec2::new(render_width, render_height),
        )))
    } else {
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
        Ok(Some((
            Vec2::new(pillarbox_size / 2., letterbox_size / 2.),
            Vec2::new(render_width, render_height),
        )))
    }
}

fn calculate_aspect_ratio_from_letterbox(
    physical_size: &Vec2,
    letterbox: (&u32, &u32),
) -> Option<(Vec2, Vec2)> {
    let letterbox_height = (letterbox.0 + letterbox.1) as f32;
    let render_width = physical_size.x;
    let aspect_ratio = render_width / (physical_size.y - letterbox_height);
    let render_height = render_width / aspect_ratio;

    Some((
        Vec2::new(0., *letterbox.0 as f32),
        Vec2::new(render_width, render_height),
    ))
}

fn calculate_aspect_ratio_from_pillarbox(
    physical_size: &Vec2,
    pillarbox: (&u32, &u32),
) -> Option<(Vec2, Vec2)> {
    let pillarbox_width = (pillarbox.0 + pillarbox.1) as f32;
    let render_height = physical_size.y;
    let aspect_ratio = (physical_size.x + pillarbox_width) / render_height;
    let render_width = render_height / aspect_ratio;

    Some((
        Vec2::new(*pillarbox.0 as f32, 0.),
        Vec2::new(render_width, render_height),
    ))
}
