use bevy::math::AspectRatio;
use bevy::prelude::*;
use bevy::render::camera::{ManualTextureView, ManualTextureViews, NormalizedRenderTarget, RenderTarget, Viewport, ScalingMode};
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::window::{PrimaryWindow, WindowRef};

pub struct LetterboxPlugin;
impl Plugin for LetterboxPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<CameraBox>()
            .add_systems(Update, adjust_viewport);
    }
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct CameraBox {
    pub mode: CameraBoxMode, // Rename?
}

#[derive(Reflect)]
pub enum CameraBoxMode {
    StaticResolution {
        resolution: UVec2,
        position: Option<UVec2>,
    },
    StaticAspectRatio {
        aspect_ratio: AspectRatio,
        position: Option<UVec2>,
    },
    ResolutionIntegerScale {
        allow_imperfect_aspect_ratios: bool,
        resolution: Vec2,
    },
    LetterBox { top_size: UVec2, bottom_size: UVec2 },
    PillarBox { top_size: UVec2, bottom_size: UVec2 },
}
fn adjust_viewport(
    mut boxed_cameras: Query<(&mut Camera, &CameraBox)>,
    primary_window: Query<Option<Entity>, With<PrimaryWindow>>,
    windows: Query<(Entity, &Window)>,
    texture_views: Res<ManualTextureViews>,
    images: Res<Assets<Image>>,
) {
    for (mut camera, camera_box) in boxed_cameras.iter_mut() {
        match &camera_box.mode {
            CameraBoxMode::StaticResolution { resolution: size, position, } => match &mut camera.viewport {
                Some(viewport) => {
                    if &viewport.physical_size != size {
                        if size.x > viewport.physical_size.x || size.y > viewport.physical_size.y {
                            viewport.physical_size = size.clone();
                        } else {
                            viewport.clamp_to_size(size.clone());
                        }
                    }
                    if position
                        .is_some_and(|u| u != viewport.physical_position)
                    {
                        viewport.physical_position = position.unwrap().clone();
                    } else if position.is_none() {
                        viewport.physical_position = default();
                    }
                }
                None => {
                    camera.viewport = Some(Viewport {
                        physical_size: size.clone(),
                        physical_position: if position.is_some() {
                            position.unwrap()
                        } else {
                            Default::default()
                        },
                        depth: Default::default(),
                        ..default()
                    })
                }
            },
            CameraBoxMode::StaticAspectRatio { aspect_ratio, position } => match &mut camera.viewport {
                _ => todo!()
            },
            CameraBoxMode::ResolutionIntegerScale { allow_imperfect_aspect_ratios, resolution }=> {
                let target = camera.target.normalize(
                    primary_window
                        .iter()
                        .collect::<Vec<Option<Entity>>>()
                        .first()
                        .unwrap()
                        .to_owned(),
                ); // Probably a better way to do this.

                let target = match target.and_then(|t| t.get_render_target_info(windows, &images, &texture_views)) {
                    None => continue,
                    Some(target) => target
                };

                let (boxing, sizing) =  if *allow_imperfect_aspect_ratios {
                    match calculate_sizes_imperfect(&target.physical_size.as_vec2(), resolution) {
                        Ok(opt) => match opt {
                            None => {
                                camera.viewport = None;
                                continue;
                            }
                            Some(t) => t,
                        }
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
                        }
                        Err(_) => continue,
                    }
                };

                camera.viewport = Some(Viewport {
                    physical_position: boxing.as_uvec2(),
                    physical_size: sizing.as_uvec2(),
                    ..default()
                });
            },
            CameraBoxMode::LetterBox { top_size, bottom_size }  => {
                todo!()
            },
            CameraBoxMode::PillarBox { top_size, bottom_size } => {
                todo!()
            }
        }
    }
}

fn calculate_sizes_imperfect(physical_size: &Vec2, desired_size: &Vec2) -> Result<Option<(Vec2, Vec2)>, ()> {
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

    let has_int_scale = desired_ar.ratio() == physical_ar.ratio() && (
        (height_scale % 1. == 0. && width_scale % 1. == 0.) || (small_height_scale % 1. == 0. && small_width_scale % 1. == 0.)
    );

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

    let letterbox_size = physical_size.y as f32 - render_height;
    let pillarbox_size = physical_size.x as f32 - render_width;

    Ok(Some((Vec2::new(pillarbox_size / 2., letterbox_size / 2.), Vec2::new(render_width, render_height))))
}

fn calculate_sizes_perfect(physical_size: &Vec2, desired_size: &Vec2) -> Result<Option<(Vec2, Vec2)>, ()> {
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

    let has_int_scale = desired_ar.ratio() == physical_ar.ratio() && (
        (height_scale % 1. == 0. && width_scale % 1. == 0.)
    );

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
        }.ceil();

        let render_height = desired_size.y / best_divisor;
        let render_width = desired_size.x / best_divisor;

        let letterbox_size = physical_size.y - render_height;
        let pillarbox_size = physical_size.x - render_width;
        Ok(Some((Vec2::new(pillarbox_size / 2., letterbox_size / 2.), Vec2::new(render_width, render_height))))
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
        Ok(Some((Vec2::new(pillarbox_size / 2., letterbox_size / 2.), Vec2::new(render_width, render_height))))
    }
}
