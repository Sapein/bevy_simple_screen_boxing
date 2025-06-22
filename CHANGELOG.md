# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [Unreleased]
- Rename `CameraBox::ResolutionIntegerScale.allow_imperfect_aspect_ratios` to `CameraBox::ResolutionIntegerScale.allow_imperfect_downscaled_boxing` as it's a better name.
- Implement `strict_pillarboxing` on `CameraBox::PillarBox`

## [0.1.1] - 2025-06-22  
- Update Documentation
- Simplify the Math for Static Letterbox.
- Rewrite the Math for Static Pillarbox, as it was wrong.
- Simplifies a lot of internal code
- Adds in various tests for internal components to ensure things are working properly.

## [0.1.0] - 2025-06-14  
- Initial Release
