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
    ResolutionIntegerScale,
    LetterBox { top_size: UVec2, bottom_size: UVec2 },
    PillarBox { top_size: UVec2, bottom_size: UVec2 },
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera2d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::linear_rgba(00.1, 00.1, 0.6, 0.0)),
            order: 2,
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMax {
                max_width: 640.,
                max_height: 360.,
            },
            far: 1000.,
            near: -1000.,
            scale: 0.5,
            viewport_origin: Vec2::new(0.5, 0.5),
            area: Default::default(),
        }),
        CameraBox {
            mode: CameraBoxMode::ResolutionIntegerScale,
            // mode: CameraBoxMode::StaticSize {
            //     resolution: UVec2::new(640, 360),
            //     position: None,
            // }
        },
    ));

    // commands.spawn((
    //     Camera2d::default(),
    //     Camera {
    //         order: 1,
    //         clear_color: ClearColorConfig::Custom(Color::linear_rgba(0.1, 0.6, 0.1, 0.5)),
    //         ..default()
    //     },
    //     CameraBox {
    //         mode: CameraBoxMode::StaticSize {
    //             resolution: UVec2::new(200, 360),
    //             position: Some(UVec2::new(640, 0)),
    //         },
    //     },
    // ));
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
            CameraBoxMode::ResolutionIntegerScale => {
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
                
                match projection {
                    Projection::Perspective(_) => todo!(),
                    Projection::Orthographic(projection) => {
                        match projection.scaling_mode {
                            ScalingMode::WindowSize => {}
                            ScalingMode::Fixed { .. } => {}
                            ScalingMode::AutoMin { .. } => {}
                            ScalingMode::AutoMax { max_height: desired_height, max_width: desired_width } => {
                                let desired_ar = AspectRatio::try_new(desired_width, desired_height);
                                let physical_ar = AspectRatio::try_new(target.physical_size.x as f32, target.physical_size.y as f32);
                                if desired_ar.is_err() || physical_ar.is_err() { 
                                    continue;
                                }
                                
                                let desired_ar = desired_ar.unwrap();
                                let physical_ar = physical_ar.unwrap();

                                //NOTE: this does not really handle the case where the target size is smaller than the desired height/width.
                                let height_diff = target.physical_size.y as f32 / desired_height;
                                let width_diff = target.physical_size.x as f32 / desired_width;
                                let has_int_scale = desired_ar.ratio() == physical_ar.ratio() && (height_diff % 1. == 0. && width_diff % 1. == 0.);
                                
                                // Integer Scaling Exists
                                if has_int_scale {
                                    camera.viewport = None;
                                    continue;
                                }
                                
                                // Letterbox Calculations
                                // 
                                // 1280x720 (AR of 16:9, or 1.777...)
                                // Target AR of 4:3 (AR of 1.333...)
                                // 
                                //  AR = w / h
                                //  ARt = wT / h
                                //  wT = ARt / h
                                //  s = (w - wT)
                                //  lb = s / 2


                                // Note this does not handle the case where it's not bigger.
                                let render_width = desired_width * width_diff.trunc();
                                let render_height = desired_height * height_diff.trunc();
                                
                                dbg!(target.physical_size);
                                dbg!((render_height, render_width));
                                let letterbox_size = if height_diff % 1. != 0. {
                                    ((target.physical_size.y as f32 - render_height) / 1.).round()
                                } else { 
                                    0.
                                };

                                let pillarbox_size = if width_diff % 1. != 0. {
                                    ((target.physical_size.x as f32 - render_width) / 1.).round()
                                } else { 
                                    0.
                                };
                                
                                dbg!((letterbox_size, pillarbox_size));
                                let mut port = Viewport {
                                    physical_position: Vec2::new(pillarbox_size, letterbox_size).as_uvec2(),
                                    physical_size: Vec2::new(render_width, render_height).as_uvec2(),
                                    ..default()
                                };
                                port.clamp_to_size(Vec2::new(render_width, render_height).as_uvec2());
                                camera.viewport = Some(port);
                            }
                            ScalingMode::FixedVertical { .. } => {}
                            ScalingMode::FixedHorizontal { .. } => {}
                        }
                    }
                    Projection::Custom(_) => todo!(),
                }
            }
            _ => todo!(),
        }
    }
}
