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
enum CameraBoxMode {
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
    LetterBox {
        top: u32,
        bottom: u32,
        strict_letterboxing: bool,
    },
    PillarBox {
        left: u32,
        right: u32,
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

        let target = match target.and_then(|t| t.get_render_target_info(windows, &images, &texture_views)) {
            None => continue,
            Some(target) => target
        };

        match &camera_box.mode {
            CameraBoxMode::StaticResolution { resolution: size, position, } => match &mut camera.viewport {
                Some(viewport) => {
                    if &viewport.physical_size != size {
                        if size.x > viewport.physical_size.x || size.y > viewport.physical_size.y {
                            viewport.physical_size = size.clone();
                            if target.physical_size.x < size.x {
                                viewport.physical_size.x = target.physical_size.x;
                            }
                            if target.physical_size.y < size.y {
                                viewport.physical_size.y = target.physical_size.y;
                            }
                        } else {
                            viewport.clamp_to_size(size.clone());
                        }
                    }
                    if position
                        .is_some_and(|u| u != viewport.physical_position)
                    {
                        viewport.physical_position = position.unwrap().clone();
                    } else if position.is_none() {
                        viewport.physical_position = (target.physical_size - viewport.physical_size) / 2;
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
                None => {
                    let physical_aspect_ratio = match AspectRatio::try_from(target.physical_size.as_vec2()) {
                        Ok(ar) => ar,
                        Err(_) => continue,
                    };
                    if physical_aspect_ratio.ratio() == aspect_ratio.ratio() {
                        camera.viewport = None;
                        continue;
                    }
                    let (boxing, sizing) = calculate_sizes_resolution(&target.physical_size.as_vec2(), aspect_ratio).unwrap();
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
                    let physical_aspect_ratio = match AspectRatio::try_from(target.physical_size.as_vec2()) {
                        Ok(ar) => ar,
                        Err(_) => continue,
                    };
                    if physical_aspect_ratio.ratio() == aspect_ratio.ratio() {
                        camera.viewport = None;
                        continue;
                    }
                    let (boxing, sizing) = calculate_sizes_resolution(&target.physical_size.as_vec2(), aspect_ratio).unwrap();
                    viewport.physical_position = boxing.as_uvec2();
                    viewport.physical_size = match position {
                        None => sizing.as_uvec2(),
                        Some(pos) => *pos,
                    }
                }
            },
            CameraBoxMode::ResolutionIntegerScale { allow_imperfect_aspect_ratios, resolution }=> {
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
            CameraBoxMode::LetterBox { top, bottom, strict_letterboxing } => match &mut camera.viewport {
                None => {
                    let (mut boxing, mut sizing) = calculate_aspect_ratio_from_letterbox(&target.physical_size.as_vec2(), (top, bottom)).unwrap();
                    if (sizing.y + boxing.y > target.physical_size.y as f32 || sizing.y <= 0.) && !strict_letterboxing{
                        sizing.y = target.physical_size.y as f32 / 2.;
                        boxing.y /= 2.;
                        let scale_factor = (target.physical_size.y as f32) / (sizing.y + boxing.y);
                        boxing.y *= scale_factor;
                    }

                    if (sizing.y <= 0. || sizing.y > target.physical_size.y as f32 || sizing.y + boxing.y > target.physical_size.y as f32) && *strict_letterboxing {
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
                    let (mut boxing, mut sizing) = calculate_aspect_ratio_from_letterbox(&target.physical_size.as_vec2(), (top, bottom)).unwrap();

                    if (sizing.y + boxing.y > target.physical_size.y as f32 || sizing.y <= 0.) && !strict_letterboxing{
                        sizing.y = target.physical_size.y as f32 / 2.;
                        boxing.y /= 2.;
                        let scale_factor = (target.physical_size.y as f32) / (sizing.y + boxing.y);
                        boxing.y *= scale_factor;
                    }

                    if (sizing.y <= 0. || sizing.y > target.physical_size.y as f32 || sizing.y + boxing.y > target.physical_size.y as f32) && *strict_letterboxing {
                        sizing.y = target.physical_size.y as f32;
                        sizing.x = target.physical_size.x as f32;
                        boxing.y = 0.;
                    }

                    viewport.physical_position = boxing.as_uvec2();
                    viewport.physical_size = sizing.as_uvec2();
                }
            },
            CameraBoxMode::PillarBox { left, right } => match &mut camera.viewport {
                None => {
                    let (mut boxing, mut sizing) = calculate_aspect_ratio_from_pillarbox(&target.physical_size.as_vec2(), (left, right)).unwrap();

                    if sizing.x <= 0. || sizing.x > target.physical_size.x as f32 || sizing.x + boxing.x > target.physical_size.x as f32 {
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
                    let (mut boxing, mut sizing) = calculate_aspect_ratio_from_pillarbox(&target.physical_size.as_vec2(), (left, right)).unwrap();

                    if sizing.x <= 0. || sizing.x > target.physical_size.x as f32 || sizing.x + boxing.x > target.physical_size.x as f32 {
                        sizing.x = target.physical_size.x as f32;
                        sizing.x = target.physical_size.x as f32;
                        boxing.x = 0.;
                    }

                    viewport.physical_position = boxing.as_uvec2();
                    viewport.physical_size = sizing.as_uvec2();
                }

            }
        }
    }
}

fn calculate_sizes_resolution(physical_size: &Vec2, target_aspect_ratio: &AspectRatio) -> Option<(Vec2, Vec2)> {
    let physical_aspect_ratio = AspectRatio::try_from(*physical_size);
    if physical_aspect_ratio.is_err() {
        return None;
    }
    let physical_aspect_ratio = physical_aspect_ratio.unwrap();

    if physical_aspect_ratio.ratio() > target_aspect_ratio.ratio() {
        let render_height = physical_size.y;
        let render_width = render_height * target_aspect_ratio.ratio();
        Some((Vec2::new(physical_size.x / 2. - render_width / 2., 0.), Vec2::new(render_width, render_height)))
    } else {
        let render_width = physical_size.x;
        let render_height = render_width / target_aspect_ratio.ratio();
        Some((Vec2::new(0., physical_size.y / 2. - render_height / 2.), Vec2::new(render_width, render_height)))
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

fn calculate_aspect_ratio_from_letterbox(physical_size: &Vec2, letterbox: (&u32, &u32)) -> Option<(Vec2, Vec2)> {
    let letterbox_height = (letterbox.0 + letterbox.1) as f32;
    let render_width = physical_size.x;
    let aspect_ratio = render_width / (physical_size.y - letterbox_height);
    let render_height = render_width / aspect_ratio;

    Some((Vec2::new(0., *letterbox.0 as f32), Vec2::new(render_width, render_height)))
}

fn calculate_aspect_ratio_from_pillarbox(physical_size: &Vec2, pillarbox: (&u32, &u32)) -> Option<(Vec2, Vec2)> {
    let pillarbox_width = (pillarbox.0 + pillarbox.1) as f32;
    let render_height = physical_size.y;
    let aspect_ratio = (physical_size.x + pillarbox_width) / render_height;
    let render_width = render_height / aspect_ratio;

    Some((Vec2::new(*pillarbox.0 as f32, 0.), Vec2::new(render_width, render_height)))
}
