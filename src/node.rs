use std::{collections::HashMap, sync::Mutex};

use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_resource::{
            Extent3d, Origin3d, TexelCopyTextureInfo, Texture, TextureAspect, TextureDescriptor,
            TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};
use etagere::{euclid::Size2D, AllocId, Allocation, AtlasAllocator, Rectangle};
use vello::{kurbo::Affine, AaConfig, AaSupport, RenderParams, Renderer, RendererOptions};

use crate::components::VelloScene;

struct VelloAtlas {
    atlas_alloc: AtlasAllocator,
    alloc_ids: HashMap<Entity, AllocId>,
}

impl VelloAtlas {
    fn required_size(sizes: &[(Entity, u32, u32)]) -> u32 {
        let total_area: u32 = sizes.iter().map(|(_, width, height)| width * height).sum();

        let theoretical_min_size = (total_area.max(1) as f32).sqrt().ceil() as u32;
        let mut size = theoretical_min_size.next_power_of_two();

        if size * size < total_area * 2 {
            size *= 2;
        }

        size
    }

    fn new(sizes: &[(Entity, u32, u32)]) -> Self {
        let size = Self::required_size(sizes);

        Self {
            atlas_alloc: AtlasAllocator::new(Size2D::new(size as i32, size as i32)),
            alloc_ids: HashMap::new(),
        }
    }

    fn width(&self) -> u32 {
        self.atlas_alloc.size().width as _
    }

    fn height(&self) -> u32 {
        self.atlas_alloc.size().height as _
    }

    fn resize(&mut self, size: u32) {
        self.alloc_ids.clear();
        self.atlas_alloc = AtlasAllocator::new(Size2D::new(size as i32, size as i32));
    }

    fn update_size(&mut self, sizes: &[(Entity, u32, u32)]) {
        let required_size = Self::required_size(sizes);
        let current_size = self.atlas_alloc.size().width as u32;
        let current_area = current_size * current_size;

        if !(current_area / 4..=current_area).contains(&(required_size * required_size)) {
            self.resize(required_size);
        }
    }

    fn allocate_all(&mut self, sizes: &[(Entity, u32, u32)]) {
        let mut was_resized;

        loop {
            was_resized = false;

            for (entity, width, height) in sizes {
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    self.alloc_ids.entry(*entity)
                {
                    if let Some(Allocation { id, .. }) = self
                        .atlas_alloc
                        .allocate(Size2D::new(*width as i32, *height as i32))
                    {
                        entry.insert(id);
                    } else {
                        self.resize(2 * self.atlas_alloc.size().width as u32);
                        was_resized = true;
                        break;
                    }
                }
            }

            if !was_resized {
                break;
            }
        }
    }

    fn get(&self, entity: Entity) -> Rectangle {
        self.atlas_alloc.get(self.alloc_ids[&entity])
    }
}

#[derive(Resource)]
pub struct VelloContext {
    inner: Mutex<VelloContextInner>,
}

struct VelloContextInner {
    renderer: Renderer,
    atlas: Option<VelloAtlas>,
    atlas_texture: Texture,
    has_rendered_this_frame: bool,
}

impl VelloContext {
    pub fn reset_renderer(&self) {
        self.inner.lock().unwrap().has_rendered_this_frame = false;
    }
}

impl FromWorld for VelloContext {
    fn from_world(world: &mut World) -> Self {
        let device = world.resource::<RenderDevice>();

        Self {
            inner: Mutex::new(VelloContextInner {
                renderer: Renderer::new(
                    device.wgpu_device(),
                    RendererOptions {
                        antialiasing_support: AaSupport::area_only(),
                        ..Default::default()
                    },
                )
                .expect("Failed to create Vello renderer."),
                atlas: None,
                atlas_texture: device.create_texture(&TextureDescriptor {
                    label: None,
                    size: Extent3d::default(),
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba8Unorm,
                    usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
                    view_formats: &[],
                }),
                has_rendered_this_frame: false,
            }),
        }
    }
}

pub fn render_vello_scene_textures(
    query: Query<(Entity, &VelloScene)>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    mut render_context: RenderContext,
    context: Res<VelloContext>,
) {
    let mut context = context.inner.lock().unwrap();

    if context.has_rendered_this_frame {
        return;
    }

    let scenes: Vec<_> = query
        .iter()
        .map(|(entity, scene)| {
            (
                entity,
                scene.fragment.clone(),
                scene.image_handle.clone(),
                scene.width,
                scene.height,
            )
        })
        .collect();

    if scenes.is_empty() {
        context.has_rendered_this_frame = true;
        return;
    }

    let sizes: Vec<_> = scenes
        .iter()
        .map(|(entity, _, _, width, height)| (*entity, *width, *height))
        .collect();

    let atlas_size = {
        let atlas = context.atlas.get_or_insert_with(|| VelloAtlas::new(&sizes));
        atlas.update_size(&sizes);
        atlas.allocate_all(&sizes);

        Extent3d {
            width: atlas.width(),
            height: atlas.height(),
            ..Default::default()
        }
    };

    if context.atlas_texture.width() != atlas_size.width
        || context.atlas_texture.height() != atlas_size.height
    {
        context.atlas_texture.destroy();
        context.atlas_texture = render_device.create_texture(&TextureDescriptor {
            label: None,
            size: atlas_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
    }

    let placements: Vec<_> = {
        let atlas = context.atlas.as_ref().unwrap();

        scenes
            .iter()
            .map(|(entity, _, image_handle, _, _)| {
                (*entity, atlas.get(*entity), image_handle.clone())
            })
            .collect()
    };

    let mut scene = vello::Scene::default();
    let mut max_size = (0, 0);

    for ((_, fragment, _, _, _), (_, rect, _)) in scenes.iter().zip(&placements) {
        scene.append(
            fragment,
            Some(Affine::translate((rect.min.x as f64, rect.min.y as f64))),
        );

        max_size.0 = max_size.0.max(rect.max.x as u32);
        max_size.1 = max_size.1.max(rect.max.y as u32);
    }

    let atlas_texture_view = context
        .atlas_texture
        .create_view(&TextureViewDescriptor::default());

    context
        .renderer
        .render_to_texture(
            render_device.wgpu_device(),
            &render_queue,
            &scene,
            &atlas_texture_view,
            &RenderParams {
                base_color: vello::peniko::Color::TRANSPARENT,
                width: max_size.0,
                height: max_size.1,
                antialiasing_method: AaConfig::Area,
            },
        )
        .expect("Failed to render with Vello.");

    for (_entity, rect, image_handle) in &placements {
        let gpu_image = gpu_images
            .get(image_handle.id())
            .expect("The Vello output image must exist on the GPU.");

        render_context.command_encoder().copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: &context.atlas_texture,
                mip_level: 0,
                origin: Origin3d {
                    x: rect.min.x as u32,
                    y: rect.min.y as u32,
                    ..Default::default()
                },
                aspect: TextureAspect::All,
            },
            TexelCopyTextureInfo {
                texture: &gpu_image.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            gpu_image.texture_descriptor.size,
        );
    }

    context.has_rendered_this_frame = true;
}
