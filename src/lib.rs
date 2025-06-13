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
    StaticSize {
        resolution: UVec2,
        position: Option<UVec2>,
    },
    // StaticAspectRatio(AspectRatio),
    ResolutionIntegerScale {
        allow_imperfect_aspect_ratios: bool,
    },
    LetterBox { top_size: UVec2, bottom_size: UVec2 },
    PillarBox { top_size: UVec2, bottom_size: UVec2 },
}

fn adjust_viewport(
    mut boxed_cameras: Query<(&mut Camera, &Projection, &CameraBox)>,
    primary_window: Query<Option<Entity>, With<PrimaryWindow>>,
    windows: Query<(Entity, &Window)>,
    texture_views: Res<ManualTextureViews>,
    images: Res<Assets<Image>>,
) {
    for (mut camera, projection, camera_box) in boxed_cameras.iter_mut() {
        match &camera_box.mode {
            CameraBoxMode::StaticSize { resolution: size, position, } => match &mut camera.viewport {
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
            CameraBoxMode::ResolutionIntegerScale { allow_imperfect_aspect_ratios }=> {
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

                if !allow_imperfect_aspect_ratios {
                    todo!()
                }

                match projection {
                    Projection::Perspective(_) => todo!(),
                    Projection::Orthographic(projection) => {
                        match projection.scaling_mode {
                            ScalingMode::WindowSize => continue,
                            ScalingMode::Fixed { .. } => todo!(),
                            ScalingMode::AutoMin { .. } => todo!(),
                            ScalingMode::AutoMax { max_height: desired_height, max_width: desired_width } => {
                                let (boxing, sizing) = match calculate_sizes_imperfect(target.physical_size, (desired_width, desired_height)) {
                                    Ok(opt) => match opt {
                                        None => {
                                            camera.viewport = None;
                                            continue;
                                        }
                                        Some((b, s)) => (b, s),
                                    }
                                    Err(_) => continue,
                                };
                                let mut port = Viewport {
                                    physical_position: boxing,
                                    physical_size: sizing,
                                    ..default()
                                };
                                camera.viewport = Some(port);
                            }
                            ScalingMode::FixedVertical { .. } => todo!(),
                            ScalingMode::FixedHorizontal { .. } => todo!(),
                        }
                    }
                    Projection::Custom(_) => todo!(),
                }
            }
            _ => todo!(),
        }
    }
}

fn calculate_sizes_imperfect(physical_size: UVec2, (desired_width, desired_height): (f32, f32)) -> Result<Option<(UVec2, UVec2)>, ()> {
    let desired_aspect_ratio = AspectRatio::try_new(desired_width, desired_height);
    let physical_aspect_ratio = AspectRatio::try_new(physical_size.x as f32, physical_size.y as f32);
    if desired_aspect_ratio.is_err() || physical_aspect_ratio.is_err() {
        return Err(());
    }

    let desired_ar = desired_aspect_ratio.unwrap();
    let physical_ar = physical_aspect_ratio.unwrap();

    //NOTE: this does not really handle the case where the target size is smaller than the desired height/width.
    let height_scale = physical_size.y as f32 / desired_height;
    let width_scale = physical_size.x as f32 / desired_width;

    let small_height_scale = desired_height / physical_size.y as f32;
    let small_width_scale = desired_height / physical_size.y as f32;

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
        desired_width * best_scale.floor()
    } else {
        desired_width * best_scale
    };

    let render_height = if best_scale >= 1. {
        desired_height * best_scale.floor()
    } else {
        desired_height * best_scale
    };

    let letterbox_size = physical_size.y as f32 - render_height;
    let pillarbox_size = physical_size.x as f32 - render_width;

    Ok(Some((Vec2::new(pillarbox_size / 2. , letterbox_size / 2.).as_uvec2(), Vec2::new(desired_width, desired_height).as_uvec2())))
}