# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]
- Fix a bug in `CameraBox::StaticResolution` which could cause a crash.
- Implement `strict_pillarboxing` on `CameraBox::PillarBox`
- Rename `CameraBox::ResolutionIntegerScale.allow_imperfect_aspect_ratios` to `CameraBox::ResolutionIntegerScale.allow_imperfect_downscaled_boxing` as it's a better name.
- Run `Assets<Image>` change detection system when the resource is changed, in addition to when `AssetEvents` is emitted.
- All Change-Detection Systems have been updated to only run if at least one entity with a `CameraBox` component exists.
- The Camera Change Detection System has been removed, as it would constantly trigger the moment the plugin boxed something.
- Setting a position that can't be accommodated in `CameraBox::StaticResolution` will set the position to (0,0) and trigger a warning.
- Setting a position that can't be accommodated in `CameraBox::StaticAspectRatio` will set the position to (0,0) and trigger a warning.
- Add in Integration Testing
- Add in `CameraBoxingSet` System Set, which has two variants, which can be used for ordering.
- Make `AdjustBoxing` event public.

## [0.1.1] - 2025-06-22  
- Added Logging instead of skipping errors and other issues.
- Update Documentation
- Simplify the Math for Static Letterbox.
- Rewrite the Math for Static Pillarbox, as it was wrong.
- Simplifies a lot of internal code
- Adds in various tests for internal components to ensure things are working properly.

## [0.1.0] - 2025-06-14  
- Initial Release
